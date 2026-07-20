use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct User {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct Creator {
    pub id: Uuid,
    pub auth_subject: String,
    pub user_sub: Option<String>,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct ContentSettings {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub theme_topic: String,
    pub style_notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct InstagramAccount {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub instagram_user_id: String,
    pub username: Option<String>,
    pub access_token_ciphertext: Option<String>,
    pub refresh_token_ciphertext: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub connection_status: String,
    pub reconnect_reason: Option<String>,
    pub connected_at: DateTime<Utc>,
    pub disconnected_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct MediaAsset {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub storage_key: String,
    pub public_url: Option<String>,
    pub source: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<String>,
    pub license: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(type_name = "post_status")]
pub enum PostStatus {
    #[serde(rename = "draft")]
    #[sqlx(rename = "draft")]
    Draft,
    #[serde(rename = "pending-review")]
    #[sqlx(rename = "pending-review")]
    PendingReview,
    #[serde(rename = "approved")]
    #[sqlx(rename = "approved")]
    Approved,
    #[serde(rename = "scheduled")]
    #[sqlx(rename = "scheduled")]
    Scheduled,
    #[serde(rename = "published")]
    #[sqlx(rename = "published")]
    Published,
    #[serde(rename = "failed")]
    #[sqlx(rename = "failed")]
    Failed,
    #[serde(rename = "rejected")]
    #[sqlx(rename = "rejected")]
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct GeneratedPost {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub instagram_account_id: Option<Uuid>,
    pub media_asset_id: Option<Uuid>,
    pub image_reference: Option<String>,
    pub header_text: String,
    pub paragraph_text: String,
    pub caption: String,
    pub status: PostStatus,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub failure_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct PostingSchedule {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub timezone: String,
    pub cadence: String,
    pub schedule_rule: Value,
    pub is_active: bool,
    pub next_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct PostQueueEntry {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub post_id: Uuid,
    pub scheduled_for: DateTime<Utc>,
    pub queue_position: i32,
    pub locked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
