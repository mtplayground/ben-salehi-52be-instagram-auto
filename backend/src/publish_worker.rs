use chrono::Utc;
use serde::Deserialize;
use thiserror::Error;
use tokio::{task::JoinHandle, time::MissedTickBehavior};

use crate::{
    db::repository::PublishablePost,
    instagram::{get_valid_instagram_credential, InstagramError},
    AppState,
};

const WORKER_INTERVAL_SECONDS: u64 = 30;
const PUBLISH_BATCH_LIMIT: i64 = 10;
const GRAPH_BASE_URL: &str = "https://graph.facebook.com/v21.0";

#[derive(Debug, Default)]
pub struct PublishWorkerReport {
    pub inspected: usize,
    pub published: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Instagram credential failed: {0}")]
    Instagram(#[from] InstagramError),
    #[error("Instagram publishing failed: {0}")]
    Publish(String),
}

#[derive(Clone)]
struct InstagramPublisher {
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct MediaContainerResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
struct MediaPublishResponse {
    id: String,
}

pub fn spawn_publish_worker(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            WORKER_INTERVAL_SECONDS,
        ));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            match run_publish_worker_once(&state).await {
                Ok(report) => {
                    if report.inspected > 0 || report.failed > 0 {
                        tracing::info!(
                            inspected = report.inspected,
                            published = report.published,
                            skipped = report.skipped,
                            failed = report.failed,
                            "publish worker completed"
                        );
                    }
                }
                Err(error) => {
                    tracing::error!(%error, "publish worker run failed");
                }
            }
        }
    })
}

pub async fn run_publish_worker_once(state: &AppState) -> Result<PublishWorkerReport, PublishError> {
    let due_posts = state
        .repository
        .list_due_publishable_posts(Utc::now(), PUBLISH_BATCH_LIMIT)
        .await?;
    let publisher = InstagramPublisher::new();
    let mut report = PublishWorkerReport {
        inspected: due_posts.len(),
        ..PublishWorkerReport::default()
    };

    for post in due_posts {
        match publish_due_post(state, &publisher, post).await {
            Ok(PublishOutcome::Published) => report.published += 1,
            Ok(PublishOutcome::Skipped) => report.skipped += 1,
            Err(error) => {
                report.failed += 1;
                tracing::error!(%error, "publish worker post failed");
            }
        }
    }

    Ok(report)
}

#[derive(Debug)]
enum PublishOutcome {
    Published,
    Skipped,
}

async fn publish_due_post(
    state: &AppState,
    publisher: &InstagramPublisher,
    post: PublishablePost,
) -> Result<PublishOutcome, PublishError> {
    if !state.repository.lock_queue_entry(post.queue_id).await? {
        return Ok(PublishOutcome::Skipped);
    }

    let Some(credential) = get_valid_instagram_credential(state, post.creator_id).await? else {
        state
            .repository
            .mark_generated_post_failed(
                post.creator_id,
                post.post_id,
                "Instagram account needs to be connected before publishing.",
            )
            .await?;
        return Ok(PublishOutcome::Skipped);
    };

    match publisher
        .publish_image(
            &credential.instagram_user_id,
            &credential.access_token,
            &post.media_url,
            &post.caption,
        )
        .await
    {
        Ok(instagram_media_id) => {
            state
                .repository
                .mark_generated_post_published(post.creator_id, post.post_id, credential.account_id)
                .await?;
            tracing::info!(
                post_id = %post.post_id,
                instagram_media_id,
                scheduled_for = %post.scheduled_for,
                "published scheduled Instagram post"
            );
            Ok(PublishOutcome::Published)
        }
        Err(error) => {
            let message = error.to_string();
            state
                .repository
                .mark_generated_post_failed(post.creator_id, post.post_id, &message)
                .await?;
            Err(error)
        }
    }
}

impl InstagramPublisher {
    fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    async fn publish_image(
        &self,
        instagram_user_id: &str,
        access_token: &str,
        image_url: &str,
        caption: &str,
    ) -> Result<String, PublishError> {
        let container_url = format!("{GRAPH_BASE_URL}/{instagram_user_id}/media");
        let container = self
            .client
            .post(container_url)
            .form(&[
                ("image_url", image_url),
                ("caption", caption),
                ("access_token", access_token),
            ])
            .send()
            .await
            .map_err(|error| PublishError::Publish(error.to_string()))?;
        let container = parse_instagram_response::<MediaContainerResponse>(container).await?;
        let publish_url = format!("{GRAPH_BASE_URL}/{instagram_user_id}/media_publish");
        let published = self
            .client
            .post(publish_url)
            .form(&[
                ("creation_id", container.id.as_str()),
                ("access_token", access_token),
            ])
            .send()
            .await
            .map_err(|error| PublishError::Publish(error.to_string()))?;
        let published = parse_instagram_response::<MediaPublishResponse>(published).await?;

        Ok(published.id)
    }
}

async fn parse_instagram_response<T>(response: reqwest::Response) -> Result<T, PublishError>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| PublishError::Publish(error.to_string()))?;

    if !status.is_success() {
        return Err(PublishError::Publish(format!(
            "Instagram API returned {status}: {body}"
        )));
    }

    serde_json::from_str(&body).map_err(|error| PublishError::Publish(error.to_string()))
}
