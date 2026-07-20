use chrono::{Duration, Utc};
use instagram_auto_backend::{
    compositor::{compose_text_overlay, ImageSource, OverlayTextInput},
    db::{
        self,
        models::PostStatus,
        repository::{
            CoreRepository, NewContentSettings, NewCreator, NewGeneratedPost,
            NewInstagramAccount, NewMediaAsset, NewPostQueueEntry, NewPostingSchedule,
        },
    },
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn review_mode_pipeline_requires_approval_before_publishing() -> TestResult {
    let harness = TestHarness::new("review-on").await?;
    let creator = harness.create_creator().await?;
    let account = harness.connect_instagram_account(creator.id).await?;
    harness
        .save_content_settings(creator.id, true)
        .await?;
    harness.save_daily_schedule(creator.id).await?;

    let post_id = harness
        .create_finished_queue_post(creator.id, PostStatus::PendingReview)
        .await?;
    let queue_before_review = harness
        .repository
        .list_calendar_posts_for_creator(creator.id)
        .await?;
    assert_eq!(queue_before_review.len(), 1);
    assert!(matches!(
        queue_before_review[0].status,
        PostStatus::PendingReview
    ));
    assert!(
        harness.due_publishable_count(creator.id).await? == 0,
        "pending review posts must not be publishable"
    );

    harness
        .repository
        .update_generated_post_status(creator.id, post_id, PostStatus::Approved)
        .await?
        .expect("approved post should exist");
    let due_posts = harness
        .repository
        .list_due_publishable_posts(Utc::now(), 1_000)
        .await?
        .into_iter()
        .filter(|post| post.creator_id == creator.id)
        .collect::<Vec<_>>();
    assert_eq!(due_posts.len(), 1);
    assert_eq!(due_posts[0].post_id, post_id);

    harness
        .repository
        .mark_generated_post_published(creator.id, post_id, account.id)
        .await?
        .expect("published post should exist");
    let queue_after_publish = harness
        .repository
        .list_calendar_posts_for_creator(creator.id)
        .await?;
    assert!(matches!(queue_after_publish[0].status, PostStatus::Published));
    assert!(queue_after_publish[0].published_at.is_some());
    assert_eq!(queue_after_publish[0].failure_message, None);

    harness.cleanup_creator(creator.id).await?;
    Ok(())
}

#[tokio::test]
async fn auto_publish_pipeline_makes_scheduled_posts_publishable() -> TestResult {
    let harness = TestHarness::new("auto-publish").await?;
    let creator = harness.create_creator().await?;
    let account = harness.connect_instagram_account(creator.id).await?;
    harness
        .save_content_settings(creator.id, false)
        .await?;
    harness.save_daily_schedule(creator.id).await?;

    let post_id = harness
        .create_finished_queue_post(creator.id, PostStatus::Scheduled)
        .await?;
    let due_posts = harness
        .repository
        .list_due_publishable_posts(Utc::now(), 1_000)
        .await?
        .into_iter()
        .filter(|post| post.creator_id == creator.id)
        .collect::<Vec<_>>();
    assert_eq!(due_posts.len(), 1);
    assert_eq!(due_posts[0].post_id, post_id);

    harness
        .repository
        .mark_generated_post_published(creator.id, post_id, account.id)
        .await?
        .expect("published post should exist");
    let queue_after_publish = harness
        .repository
        .list_calendar_posts_for_creator(creator.id)
        .await?;
    assert!(matches!(queue_after_publish[0].status, PostStatus::Published));
    assert_eq!(queue_after_publish[0].publish_retry_count, 0);
    assert!(queue_after_publish[0].next_retry_at.is_none());

    harness.cleanup_creator(creator.id).await?;
    Ok(())
}

#[tokio::test]
async fn transient_publish_failure_remains_visible_and_retries() -> TestResult {
    let harness = TestHarness::new("retry").await?;
    let creator = harness.create_creator().await?;
    let account = harness.connect_instagram_account(creator.id).await?;
    harness
        .save_content_settings(creator.id, false)
        .await?;
    harness.save_daily_schedule(creator.id).await?;

    let post_id = harness
        .create_finished_queue_post(creator.id, PostStatus::Scheduled)
        .await?;
    let retry_at = Utc::now() + Duration::minutes(2);
    harness
        .repository
        .mark_generated_post_failed(
            creator.id,
            post_id,
            "Instagram API returned 503: temporary outage",
            Some(retry_at),
        )
        .await?
        .expect("failed post should exist");

    let queue_after_failure = harness
        .repository
        .list_calendar_posts_for_creator(creator.id)
        .await?;
    assert!(matches!(queue_after_failure[0].status, PostStatus::Failed));
    assert_eq!(queue_after_failure[0].publish_retry_count, 1);
    assert!(queue_after_failure[0].failed_at.is_some());
    assert_eq!(
        queue_after_failure[0].failure_message.as_deref(),
        Some("Instagram API returned 503: temporary outage")
    );
    assert_eq!(queue_after_failure[0].next_retry_at, Some(retry_at));
    assert!(
        harness.due_publishable_count(creator.id).await? == 0,
        "failed posts should wait until next_retry_at before retrying"
    );

    harness
        .repository
        .mark_generated_post_failed(
            creator.id,
            post_id,
            "Instagram API returned 503: retry now",
            Some(Utc::now() - Duration::seconds(1)),
        )
        .await?
        .expect("failed post should exist");
    let due_retry_posts = harness
        .repository
        .list_due_publishable_posts(Utc::now(), 1_000)
        .await?
        .into_iter()
        .filter(|post| post.creator_id == creator.id)
        .collect::<Vec<_>>();
    assert_eq!(due_retry_posts.len(), 1);
    assert_eq!(due_retry_posts[0].post_id, post_id);
    assert_eq!(due_retry_posts[0].publish_retry_count, 2);

    harness
        .repository
        .mark_generated_post_published(creator.id, post_id, account.id)
        .await?
        .expect("published retry should exist");
    let queue_after_retry_publish = harness
        .repository
        .list_calendar_posts_for_creator(creator.id)
        .await?;
    assert!(matches!(
        queue_after_retry_publish[0].status,
        PostStatus::Published
    ));
    assert_eq!(queue_after_retry_publish[0].publish_retry_count, 0);
    assert!(queue_after_retry_publish[0].next_retry_at.is_none());
    assert!(queue_after_retry_publish[0].failure_message.is_none());

    harness.cleanup_creator(creator.id).await?;
    Ok(())
}

struct TestHarness {
    repository: CoreRepository,
    pool: PgPool,
    label: String,
}

impl TestHarness {
    async fn new(label: &str) -> TestResult<Self> {
        let database_url = std::env::var("DATABASE_URL").or_else(|_| {
            std::fs::read_to_string("/workspace/.database_url").map(|value| {
                value.trim().to_owned()
            })
        })?;
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await?;
        db::run_migrations(&pool).await?;
        let repository = CoreRepository::new(pool.clone());

        Ok(Self {
            repository,
            pool,
            label: label.to_owned(),
        })
    }

    async fn create_creator(&self) -> TestResult<instagram_auto_backend::db::models::Creator> {
        let suffix = Uuid::new_v4();
        let auth_subject = format!("test-{label}-{suffix}", label = self.label);
        let email = format!("test-{label}-{suffix}@example.com", label = self.label);
        let creator = self
            .repository
            .upsert_creator(NewCreator {
                auth_subject: &auth_subject,
                email: &email,
                display_name: Some("Pipeline Test Creator"),
                avatar_url: None,
            })
            .await?;

        Ok(creator)
    }

    async fn connect_instagram_account(
        &self,
        creator_id: Uuid,
    ) -> TestResult<instagram_auto_backend::db::models::InstagramAccount> {
        let account = self
            .repository
            .upsert_instagram_account(NewInstagramAccount {
                creator_id,
                instagram_user_id: "17841400000000000",
                username: Some("pipeline_test_creator"),
                access_token_ciphertext: Some("encrypted-access-token"),
                refresh_token_ciphertext: None,
                token_expires_at: Some(Utc::now() + Duration::days(30)),
            })
            .await?;

        Ok(account)
    }

    async fn save_content_settings(&self, creator_id: Uuid, review_mode: bool) -> TestResult {
        self.repository
            .upsert_content_settings(NewContentSettings {
                creator_id,
                theme_topic: "quiet studio productivity",
                style_notes: "simple flat illustration, warm but restrained",
                review_mode_enabled: review_mode,
            })
            .await?;

        Ok(())
    }

    async fn save_daily_schedule(&self, creator_id: Uuid) -> TestResult {
        self.repository
            .upsert_posting_schedule(NewPostingSchedule {
                creator_id,
                timezone: "UTC",
                cadence: "daily",
                schedule_rule: json!({
                    "time_of_day": "09:00",
                    "weekdays": [1, 2, 3, 4, 5, 6, 7],
                }),
                is_active: true,
                next_run_at: Some(Utc::now() + Duration::hours(1)),
            })
            .await?;

        Ok(())
    }

    async fn create_finished_queue_post(
        &self,
        creator_id: Uuid,
        status: PostStatus,
    ) -> TestResult<Uuid> {
        let header = "A quick studio reset";
        let paragraph = "Here is one small shift I am making before the next focused work block.";
        let caption = "I am keeping today simple: one setup tweak, one clear priority, and a little more breathing room.";
        let composed = compose_text_overlay(OverlayTextInput {
            image_source: ImageSource::Url("https://cdn.example.com/studio.png".to_owned()),
            header_text: header.to_owned(),
            paragraph_text: paragraph.to_owned(),
        })?;
        let storage_key = format!("tests/{creator_id}/{}.svg", Uuid::new_v4());
        let media = self
            .repository
            .create_media_asset(NewMediaAsset {
                creator_id,
                storage_key: &storage_key,
                public_url: Some("https://cdn.example.com/rendered-post.svg"),
                source: "e2e-test-composited",
                width: Some(composed.width as i32),
                height: Some(composed.height as i32),
                mime_type: Some(&composed.mime_type),
                license: None,
            })
            .await?;
        let scheduled_for = Utc::now() - Duration::minutes(1);
        let post = self
            .repository
            .create_generated_post(NewGeneratedPost {
                creator_id,
                instagram_account_id: None,
                media_asset_id: Some(media.id),
                image_reference: Some("generated flat studio illustration"),
                header_text: header,
                paragraph_text: paragraph,
                caption,
                status,
                scheduled_at: Some(scheduled_for),
            })
            .await?;
        self.repository
            .enqueue_post(NewPostQueueEntry {
                creator_id,
                post_id: post.id,
                scheduled_for,
                queue_position: 0,
            })
            .await?;

        Ok(post.id)
    }

    async fn due_publishable_count(&self, creator_id: Uuid) -> TestResult<usize> {
        Ok(self
            .repository
            .list_due_publishable_posts(Utc::now(), 1_000)
            .await?
            .into_iter()
            .filter(|post| post.creator_id == creator_id)
            .len())
    }

    async fn cleanup_creator(&self, creator_id: Uuid) -> TestResult {
        sqlx::query("DELETE FROM creators WHERE id = $1")
            .bind(creator_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
