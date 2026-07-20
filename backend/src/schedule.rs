use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    auth::AuthError,
    db::{
        models::PostingSchedule,
        repository::NewPostingSchedule,
    },
    AppState,
};

const DAILY_CADENCE: &str = "daily";
const WEEKLY_CADENCE: &str = "weekly";
const DEFAULT_TIME_OF_DAY: &str = "09:00";
const ALL_WEEKDAYS: [u8; 7] = [1, 2, 3, 4, 5, 6, 7];

#[derive(Debug, Serialize)]
struct ScheduleResponse {
    schedule: Option<PostingSchedulePayload>,
}

#[derive(Debug, Serialize)]
struct PostingSchedulePayload {
    id: Uuid,
    creator_id: Uuid,
    timezone: String,
    cadence: String,
    time_of_day: String,
    weekdays: Vec<u8>,
    is_active: bool,
    next_run_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct SaveScheduleRequest {
    timezone: String,
    cadence: String,
    time_of_day: String,
    weekdays: Option<Vec<u8>>,
    is_active: bool,
}

#[derive(Debug)]
struct ScheduleInput {
    timezone: String,
    cadence: String,
    time_of_day: String,
    weekdays: Vec<u8>,
    is_active: bool,
    next_run_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Error)]
enum ScheduleError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Validation(String),
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(get_schedule).put(save_schedule))
}

async fn get_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ScheduleResponse>, ScheduleError> {
    let creator = state.auth.current_creator(&headers).await?;
    let schedule = state
        .repository
        .get_posting_schedule(creator.creator.id)
        .await?
        .map(PostingSchedulePayload::from);

    Ok(Json(ScheduleResponse { schedule }))
}

async fn save_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SaveScheduleRequest>,
) -> Result<Json<ScheduleResponse>, ScheduleError> {
    let creator = state.auth.current_creator(&headers).await?;
    let input = request.validate()?;
    let schedule_rule = json!({
        "time_of_day": input.time_of_day,
        "weekdays": input.weekdays,
    });

    let schedule = state
        .repository
        .upsert_posting_schedule(NewPostingSchedule {
            creator_id: creator.creator.id,
            timezone: &input.timezone,
            cadence: &input.cadence,
            schedule_rule,
            is_active: input.is_active,
            next_run_at: input.next_run_at,
        })
        .await?;

    Ok(Json(ScheduleResponse {
        schedule: Some(schedule.into()),
    }))
}

impl SaveScheduleRequest {
    fn validate(self) -> Result<ScheduleInput, ScheduleError> {
        let timezone = normalize_text(&self.timezone);
        let parsed_timezone = parse_timezone(&timezone)?;
        let cadence = normalize_text(&self.cadence).to_lowercase();
        validate_cadence(&cadence)?;
        let parsed_time = parse_time_of_day(&self.time_of_day)?;
        let time_of_day = parsed_time.format("%H:%M").to_string();
        let weekdays = normalize_weekdays(&cadence, self.weekdays)?;
        let next_run_at = if self.is_active {
            Some(next_run_at(parsed_timezone, parsed_time, &weekdays)?)
        } else {
            None
        };

        Ok(ScheduleInput {
            timezone,
            cadence,
            time_of_day,
            weekdays,
            is_active: self.is_active,
            next_run_at,
        })
    }
}

impl From<PostingSchedule> for PostingSchedulePayload {
    fn from(schedule: PostingSchedule) -> Self {
        Self {
            id: schedule.id,
            creator_id: schedule.creator_id,
            timezone: schedule.timezone,
            cadence: schedule.cadence,
            time_of_day: schedule_time_of_day(&schedule.schedule_rule),
            weekdays: schedule_weekdays(&schedule.schedule_rule),
            is_active: schedule.is_active,
            next_run_at: schedule.next_run_at.map(|value| value.to_rfc3339()),
            created_at: schedule.created_at.to_rfc3339(),
            updated_at: schedule.updated_at.to_rfc3339(),
        }
    }
}

fn validate_cadence(cadence: &str) -> Result<(), ScheduleError> {
    match cadence {
        DAILY_CADENCE | WEEKLY_CADENCE => Ok(()),
        _ => Err(ScheduleError::Validation(
            "Cadence must be daily or weekly.".to_owned(),
        )),
    }
}

fn parse_timezone(value: &str) -> Result<Tz, ScheduleError> {
    if value.is_empty() || value.len() > 64 {
        return Err(ScheduleError::Validation(
            "Timezone must be a valid IANA timezone.".to_owned(),
        ));
    }

    value.parse::<Tz>().map_err(|_| {
        ScheduleError::Validation("Timezone must be a valid IANA timezone.".to_owned())
    })
}

fn parse_time_of_day(value: &str) -> Result<NaiveTime, ScheduleError> {
    NaiveTime::parse_from_str(value.trim(), "%H:%M").map_err(|_| {
        ScheduleError::Validation("Posting time must use HH:MM in 24-hour time.".to_owned())
    })
}

fn normalize_weekdays(
    cadence: &str,
    weekdays: Option<Vec<u8>>,
) -> Result<Vec<u8>, ScheduleError> {
    if cadence == DAILY_CADENCE {
        return Ok(ALL_WEEKDAYS.to_vec());
    }

    let Some(mut values) = weekdays else {
        return Err(ScheduleError::Validation(
            "Choose at least one posting day.".to_owned(),
        ));
    };

    values.sort_unstable();
    values.dedup();

    if values.is_empty() {
        return Err(ScheduleError::Validation(
            "Choose at least one posting day.".to_owned(),
        ));
    }

    if values.iter().any(|value| !(1..=7).contains(value)) {
        return Err(ScheduleError::Validation(
            "Posting days must be between 1 and 7.".to_owned(),
        ));
    }

    Ok(values)
}

fn next_run_at(
    timezone: Tz,
    time_of_day: NaiveTime,
    weekdays: &[u8],
) -> Result<DateTime<Utc>, ScheduleError> {
    let now_utc = Utc::now();
    let local_now = now_utc.with_timezone(&timezone);

    for day_offset in 0..14 {
        let date = local_now.date_naive() + Duration::days(day_offset);
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

        if candidate > now_utc {
            return Ok(candidate);
        }
    }

    Err(ScheduleError::Validation(
        "Could not calculate the next posting time.".to_owned(),
    ))
}

fn schedule_time_of_day(rule: &Value) -> String {
    rule.get("time_of_day")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| DEFAULT_TIME_OF_DAY.to_owned())
}

fn schedule_weekdays(rule: &Value) -> Vec<u8> {
    let weekdays = rule
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

impl IntoResponse for ScheduleError {
    fn into_response(self) -> Response {
        match self {
            ScheduleError::Auth(error) => error.into_response(),
            ScheduleError::Validation(message) => {
                (StatusCode::BAD_REQUEST, Json(error_body(message))).into_response()
            }
            ScheduleError::Database(error) => {
                tracing::error!(%error, "schedule database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(error_body("Schedule settings could not be saved.".to_owned())),
                )
                    .into_response()
            }
        }
    }
}

fn error_body(message: String) -> serde_json::Value {
    serde_json::json!({ "error": message })
}
