use chrono::{
    DateTime, Datelike, Duration, LocalResult, NaiveTime, TimeZone, Timelike, Utc,
};
use chrono_tz::Tz;
use serde_json::Value;
use thiserror::Error;
use tokio::{task::JoinHandle, time::MissedTickBehavior};

use crate::{
    compositor::{
        compose_text_overlay, CompositedImagePayload, CompositorError, ImageSource,
        OverlayTextInput,
    },
    db::{
        models::{ContentSettings, PostStatus, PostingSchedule},
        repository::{NewGeneratedPost, NewPostQueueEntry},
    },
    generation::{
        generate_casual_caption_for_creator, generate_flat_illustration_for_creator,
        GenerationError,
    },
    stock::{source_royalty_free_stock_image_for_creator, StockImageError},
    storage::{store_finished_image_for_creator, StoreFinishedImageInput, StorageError},
    AppState,
};

const WORKER_INTERVAL_SECONDS: u64 = 60;
const BUILD_LOOKAHEAD_HOURS: i64 = 48;
const SCHEDULE_BATCH_LIMIT: i64 = 10;
const DEFAULT_TIME_OF_DAY: &str = "09:00";
const ALL_WEEKDAYS: [u8; 7] = [1, 2, 3, 4, 5, 6, 7];

#[derive(Debug, Default)]
pub struct PipelineWorkerReport {
    pub inspected: usize,
    pub built: usize,
    pub skipped_existing: usize,
    pub failed: usize,
}

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("content settings are missing for creator schedule")]
    MissingContentSettings,
    #[error("scheduled run is missing")]
    MissingScheduledRun,
    #[error("stock fallback failed: {0}")]
    StockImage(StockImageError),
    #[error("caption generation failed: {0}")]
    CaptionGeneration(GenerationError),
    #[error("image compositing failed: {0}")]
    Compositor(#[from] CompositorError),
    #[error("finished image storage failed: {0}")]
    Storage(#[from] StorageError),
    #[error("{0}")]
    Schedule(String),
}

#[derive(Debug)]
struct PostCopy {
    header: String,
    paragraph: String,
    subject_hint: String,
}

#[derive(Debug)]
struct PreparedImage {
    source: ImageSource,
    reference: String,
    media_source: String,
    license: Option<String>,
}

pub fn spawn_pipeline_worker(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            WORKER_INTERVAL_SECONDS,
        ));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            match run_pipeline_worker_once(&state).await {
                Ok(report) => {
                    if report.inspected > 0 || report.failed > 0 {
                        tracing::info!(
                            inspected = report.inspected,
                            built = report.built,
                            skipped_existing = report.skipped_existing,
                            failed = report.failed,
                            "pipeline worker completed"
                        );
                    }
                }
                Err(error) => {
                    tracing::error!(%error, "pipeline worker run failed");
                }
            }
        }
    })
}

pub async fn run_pipeline_worker_once(
    state: &AppState,
) -> Result<PipelineWorkerReport, PipelineError> {
    let horizon = Utc::now() + Duration::hours(BUILD_LOOKAHEAD_HOURS);
    let schedules = state
        .repository
        .list_posting_schedules_ready_for_build(horizon, SCHEDULE_BATCH_LIMIT)
        .await?;
    let mut report = PipelineWorkerReport {
        inspected: schedules.len(),
        ..PipelineWorkerReport::default()
    };

    for schedule in schedules {
        match build_or_advance_schedule_slot(state, &schedule).await {
            Ok(BuildOutcome::Built) => report.built += 1,
            Ok(BuildOutcome::SkippedExisting) => report.skipped_existing += 1,
            Err(error) => {
                report.failed += 1;
                tracing::error!(
                    %error,
                    schedule_id = %schedule.id,
                    creator_id = %schedule.creator_id,
                    "scheduled post build failed"
                );
            }
        }
    }

    Ok(report)
}

#[derive(Debug)]
enum BuildOutcome {
    Built,
    SkippedExisting,
}

async fn build_or_advance_schedule_slot(
    state: &AppState,
    schedule: &PostingSchedule,
) -> Result<BuildOutcome, PipelineError> {
    let scheduled_for = schedule
        .next_run_at
        .ok_or(PipelineError::MissingScheduledRun)?;

    if scheduled_for <= Utc::now() {
        advance_schedule(state, schedule, Utc::now()).await?;
        return Ok(BuildOutcome::SkippedExisting);
    }

    if state
        .repository
        .queue_slot_exists(schedule.creator_id, scheduled_for)
        .await?
    {
        advance_schedule(state, schedule, scheduled_for).await?;
        return Ok(BuildOutcome::SkippedExisting);
    }

    build_scheduled_post(state, schedule, scheduled_for).await?;
    advance_schedule(state, schedule, scheduled_for).await?;
    Ok(BuildOutcome::Built)
}

async fn build_scheduled_post(
    state: &AppState,
    schedule: &PostingSchedule,
    scheduled_for: DateTime<Utc>,
) -> Result<(), PipelineError> {
    let settings = state
        .repository
        .get_content_settings(schedule.creator_id)
        .await?
        .ok_or(PipelineError::MissingContentSettings)?;
    let post_status = if settings.review_mode_enabled {
        PostStatus::PendingReview
    } else {
        PostStatus::Scheduled
    };
    let copy = build_post_copy(&settings, scheduled_for);
    let prepared_image = prepare_image(state, schedule.creator_id, &copy.subject_hint).await?;
    let caption = generate_casual_caption_for_creator(
        state,
        schedule.creator_id,
        &copy.header,
        &copy.paragraph,
        Some(&copy.subject_hint),
    )
    .await
    .map_err(PipelineError::CaptionGeneration)?;
    let composited = compose_text_overlay(OverlayTextInput {
        image_source: prepared_image.source,
        header_text: copy.header.clone(),
        paragraph_text: copy.paragraph.clone(),
    })?;
    let media = store_composited_image(
        state,
        schedule.creator_id,
        composited,
        prepared_image.media_source,
        prepared_image.license,
    )
    .await?;
    let post = state
        .repository
        .create_generated_post(NewGeneratedPost {
            creator_id: schedule.creator_id,
            instagram_account_id: None,
            media_asset_id: Some(media.id),
            image_reference: Some(&prepared_image.reference),
            header_text: &copy.header,
            paragraph_text: &copy.paragraph,
            caption: &caption.caption,
            status: post_status,
            scheduled_at: Some(scheduled_for),
        })
        .await?;
    state
        .repository
        .enqueue_post(NewPostQueueEntry {
            creator_id: schedule.creator_id,
            post_id: post.id,
            scheduled_for,
            queue_position: 0,
        })
        .await?;

    Ok(())
}

async fn prepare_image(
    state: &AppState,
    creator_id: uuid::Uuid,
    subject_hint: &str,
) -> Result<PreparedImage, PipelineError> {
    match generate_flat_illustration_for_creator(state, creator_id, Some(subject_hint)).await {
        Ok(image) => {
            let reference = image.revised_prompt.unwrap_or(image.prompt);
            Ok(PreparedImage {
                source: ImageSource::DataUri {
                    mime_type: image.mime_type,
                    image_base64: image.image_base64,
                },
                reference,
                media_source: "generated-illustration".to_owned(),
                license: None,
            })
        }
        Err(error) => {
            tracing::warn!(%error, creator_id = %creator_id, "illustration generation failed, trying stock fallback");
            let stock = source_royalty_free_stock_image_for_creator(
                state,
                creator_id,
                Some(subject_hint),
            )
            .await
            .map_err(PipelineError::StockImage)?;

            Ok(PreparedImage {
                source: ImageSource::Url(stock.image_url),
                reference: stock.source_url,
                media_source: format!("{}-stock-overlay", stock.provider),
                license: Some(stock.license_url),
            })
        }
    }
}

async fn store_composited_image(
    state: &AppState,
    creator_id: uuid::Uuid,
    image: CompositedImagePayload,
    source: String,
    license: Option<String>,
) -> Result<crate::db::models::MediaAsset, PipelineError> {
    let width = i32::try_from(image.width).map_err(|_| {
        PipelineError::Schedule("Composited image width is too large.".to_owned())
    })?;
    let height = i32::try_from(image.height).map_err(|_| {
        PipelineError::Schedule("Composited image height is too large.".to_owned())
    })?;
    let (media, _) = store_finished_image_for_creator(
        state,
        creator_id,
        StoreFinishedImageInput {
            image_base64: image.image_base64,
            mime_type: image.mime_type,
            source: Some(source),
            license,
            width: Some(width),
            height: Some(height),
            post_id: None,
        },
    )
    .await?;

    Ok(media)
}

async fn advance_schedule(
    state: &AppState,
    schedule: &PostingSchedule,
    scheduled_for: DateTime<Utc>,
) -> Result<(), PipelineError> {
    let next_run_at = next_run_after(schedule, scheduled_for + Duration::seconds(1))?;
    state
        .repository
        .update_posting_schedule_next_run(schedule.id, Some(next_run_at))
        .await?;

    Ok(())
}

fn build_post_copy(settings: &ContentSettings, scheduled_for: DateTime<Utc>) -> PostCopy {
    let theme = normalize_text(&settings.theme_topic);
    let date = scheduled_for.format("%b %d").to_string();
    let subject_hint = truncate_to_chars(format!("{theme} idea for {date}"), 120);
    let header = truncate_to_chars(format!("A quick note on {theme}"), 64);
    let paragraph = truncate_to_chars(
        format!("Here is one simple thought I want to share about {theme} today."),
        220,
    );

    PostCopy {
        header,
        paragraph,
        subject_hint,
    }
}

fn next_run_after(
    schedule: &PostingSchedule,
    after_utc: DateTime<Utc>,
) -> Result<DateTime<Utc>, PipelineError> {
    let timezone = schedule.timezone.parse::<Tz>().map_err(|_| {
        PipelineError::Schedule("Schedule timezone is not a valid IANA timezone.".to_owned())
    })?;
    let time_of_day = schedule_time_of_day(&schedule.schedule_rule)?;
    let weekdays = schedule_weekdays(schedule);
    let local_after = after_utc.with_timezone(&timezone);

    for day_offset in 0..21 {
        let date = local_after.date_naive() + Duration::days(day_offset);
        let weekday = date.weekday().number_from_monday() as u8;
        if !weekdays.contains(&weekday) {
            continue;
        }

        let Some(naive) = date.and_hms_opt(
            time_of_day.hour(),
            time_of_day.minute(),
            time_of_day.second(),
        ) else {
            continue;
        };
        let candidate = match timezone.from_local_datetime(&naive) {
            LocalResult::Single(value) => value.with_timezone(&Utc),
            LocalResult::Ambiguous(earliest, _) => earliest.with_timezone(&Utc),
            LocalResult::None => continue,
        };

        if candidate > after_utc {
            return Ok(candidate);
        }
    }

    Err(PipelineError::Schedule(
        "Could not calculate the next scheduled run.".to_owned(),
    ))
}

fn schedule_time_of_day(rule: &Value) -> Result<NaiveTime, PipelineError> {
    let value = rule
        .get("time_of_day")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_TIME_OF_DAY);
    NaiveTime::parse_from_str(value, "%H:%M").map_err(|_| {
        PipelineError::Schedule("Schedule time_of_day is not valid HH:MM.".to_owned())
    })
}

fn schedule_weekdays(schedule: &PostingSchedule) -> Vec<u8> {
    if schedule.cadence == "daily" {
        return ALL_WEEKDAYS.to_vec();
    }

    let weekdays = schedule
        .schedule_rule
        .get("weekdays")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|value| u8::try_from(value).ok())
                .filter(|value| (1..=7).contains(value))
                .collect::<Vec<u8>>()
        })
        .unwrap_or_default();

    if weekdays.is_empty() {
        ALL_WEEKDAYS.to_vec()
    } else {
        weekdays
    }
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn truncate_to_chars(value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value;
    }

    value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>()
        .trim_end()
        .to_owned()
        + "..."
}
