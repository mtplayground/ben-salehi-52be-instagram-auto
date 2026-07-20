use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::models::{
    Creator, GeneratedPost, InstagramAccount, MediaAsset, PostQueueEntry, PostStatus, User,
    PostingSchedule,
};

#[derive(Clone)]
pub struct CoreRepository {
    pool: PgPool,
}

#[derive(Clone, Debug)]
pub struct NewCreator<'a> {
    pub auth_subject: &'a str,
    pub email: &'a str,
    pub display_name: Option<&'a str>,
    pub avatar_url: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct NewInstagramAccount<'a> {
    pub creator_id: Uuid,
    pub instagram_user_id: &'a str,
    pub username: Option<&'a str>,
    pub access_token_ciphertext: Option<&'a str>,
    pub refresh_token_ciphertext: Option<&'a str>,
    pub token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct NewMediaAsset<'a> {
    pub creator_id: Uuid,
    pub storage_key: &'a str,
    pub public_url: Option<&'a str>,
    pub source: &'a str,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<&'a str>,
    pub license: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct NewGeneratedPost<'a> {
    pub creator_id: Uuid,
    pub instagram_account_id: Option<Uuid>,
    pub media_asset_id: Option<Uuid>,
    pub image_reference: Option<&'a str>,
    pub header_text: &'a str,
    pub paragraph_text: &'a str,
    pub caption: &'a str,
    pub status: PostStatus,
    pub scheduled_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct NewPostingSchedule<'a> {
    pub creator_id: Uuid,
    pub timezone: &'a str,
    pub cadence: &'a str,
    pub schedule_rule: Value,
    pub is_active: bool,
    pub next_run_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct NewPostQueueEntry {
    pub creator_id: Uuid,
    pub post_id: Uuid,
    pub scheduled_for: DateTime<Utc>,
    pub queue_position: i32,
}

#[derive(Clone, Debug)]
pub struct NewAuthenticatedCreator<'a> {
    pub sub: &'a str,
    pub email: &'a str,
    pub email_verified: bool,
    pub name: Option<&'a str>,
    pub picture_url: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct AuthenticatedCreator {
    pub user: User,
    pub creator: Creator,
    pub is_new_registration: bool,
}

#[derive(sqlx::FromRow)]
struct UpsertedUser {
    sub: String,
    email: String,
    name: Option<String>,
    picture_url: Option<String>,
    email_verified: bool,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    is_new_registration: bool,
}

impl CoreRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert_creator(&self, creator: NewCreator<'_>) -> Result<Creator, sqlx::Error> {
        sqlx::query_as::<_, Creator>(
            r#"
            INSERT INTO creators (auth_subject, email, display_name, avatar_url)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (auth_subject) DO UPDATE
            SET email = EXCLUDED.email,
                display_name = EXCLUDED.display_name,
                avatar_url = EXCLUDED.avatar_url,
                updated_at = NOW()
            RETURNING
                id,
                auth_subject,
                user_sub,
                email,
                display_name,
                avatar_url,
                created_at,
                updated_at
            "#,
        )
        .bind(creator.auth_subject)
        .bind(creator.email)
        .bind(creator.display_name)
        .bind(creator.avatar_url)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_creator(&self, creator_id: Uuid) -> Result<Option<Creator>, sqlx::Error> {
        sqlx::query_as::<_, Creator>(
            r#"
            SELECT
                id,
                auth_subject,
                user_sub,
                email,
                display_name,
                avatar_url,
                created_at,
                updated_at
            FROM creators
            WHERE id = $1
            "#,
        )
        .bind(creator_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn upsert_authenticated_creator(
        &self,
        creator: NewAuthenticatedCreator<'_>,
    ) -> Result<AuthenticatedCreator, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let user = upsert_user_in_tx(&mut tx, &creator).await?;
        let creator_record = upsert_creator_for_user_in_tx(&mut tx, &creator).await?;
        tx.commit().await?;

        Ok(AuthenticatedCreator {
            is_new_registration: user.is_new_registration,
            user: User {
                sub: user.sub,
                email: user.email,
                name: user.name,
                picture_url: user.picture_url,
                email_verified: user.email_verified,
                created_at: user.created_at,
                last_seen_at: user.last_seen_at,
            },
            creator: creator_record,
        })
    }

    pub async fn upsert_instagram_account(
        &self,
        account: NewInstagramAccount<'_>,
    ) -> Result<InstagramAccount, sqlx::Error> {
        sqlx::query_as::<_, InstagramAccount>(
            r#"
            INSERT INTO instagram_accounts (
                creator_id,
                instagram_user_id,
                username,
                access_token_ciphertext,
                refresh_token_ciphertext,
                token_expires_at,
                connection_status,
                connected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, 'connected', NOW())
            ON CONFLICT (creator_id, instagram_user_id) DO UPDATE
            SET username = EXCLUDED.username,
                access_token_ciphertext = EXCLUDED.access_token_ciphertext,
                refresh_token_ciphertext = EXCLUDED.refresh_token_ciphertext,
                token_expires_at = EXCLUDED.token_expires_at,
                connection_status = 'connected',
                reconnect_reason = NULL,
                disconnected_at = NULL,
                updated_at = NOW()
            RETURNING
                id,
                creator_id,
                instagram_user_id,
                username,
                access_token_ciphertext,
                refresh_token_ciphertext,
                token_expires_at,
                connection_status,
                reconnect_reason,
                connected_at,
                disconnected_at,
                created_at,
                updated_at
            "#,
        )
        .bind(account.creator_id)
        .bind(account.instagram_user_id)
        .bind(account.username)
        .bind(account.access_token_ciphertext)
        .bind(account.refresh_token_ciphertext)
        .bind(account.token_expires_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn create_media_asset(
        &self,
        asset: NewMediaAsset<'_>,
    ) -> Result<MediaAsset, sqlx::Error> {
        sqlx::query_as::<_, MediaAsset>(
            r#"
            INSERT INTO media_assets (
                creator_id,
                storage_key,
                public_url,
                source,
                width,
                height,
                mime_type,
                license
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id,
                creator_id,
                storage_key,
                public_url,
                source,
                width,
                height,
                mime_type,
                license,
                created_at
            "#,
        )
        .bind(asset.creator_id)
        .bind(asset.storage_key)
        .bind(asset.public_url)
        .bind(asset.source)
        .bind(asset.width)
        .bind(asset.height)
        .bind(asset.mime_type)
        .bind(asset.license)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn create_generated_post(
        &self,
        post: NewGeneratedPost<'_>,
    ) -> Result<GeneratedPost, sqlx::Error> {
        sqlx::query_as::<_, GeneratedPost>(
            r#"
            INSERT INTO generated_posts (
                creator_id,
                instagram_account_id,
                media_asset_id,
                image_reference,
                header_text,
                paragraph_text,
                caption,
                status,
                scheduled_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING
                id,
                creator_id,
                instagram_account_id,
                media_asset_id,
                image_reference,
                header_text,
                paragraph_text,
                caption,
                status,
                scheduled_at,
                published_at,
                failed_at,
                failure_message,
                created_at,
                updated_at
            "#,
        )
        .bind(post.creator_id)
        .bind(post.instagram_account_id)
        .bind(post.media_asset_id)
        .bind(post.image_reference)
        .bind(post.header_text)
        .bind(post.paragraph_text)
        .bind(post.caption)
        .bind(post.status)
        .bind(post.scheduled_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn upsert_posting_schedule(
        &self,
        schedule: NewPostingSchedule<'_>,
    ) -> Result<PostingSchedule, sqlx::Error> {
        sqlx::query_as::<_, PostingSchedule>(
            r#"
            INSERT INTO posting_schedules (
                creator_id,
                timezone,
                cadence,
                schedule_rule,
                is_active,
                next_run_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (creator_id) DO UPDATE
            SET timezone = EXCLUDED.timezone,
                cadence = EXCLUDED.cadence,
                schedule_rule = EXCLUDED.schedule_rule,
                is_active = EXCLUDED.is_active,
                next_run_at = EXCLUDED.next_run_at,
                updated_at = NOW()
            RETURNING
                id,
                creator_id,
                timezone,
                cadence,
                schedule_rule,
                is_active,
                next_run_at,
                created_at,
                updated_at
            "#,
        )
        .bind(schedule.creator_id)
        .bind(schedule.timezone)
        .bind(schedule.cadence)
        .bind(schedule.schedule_rule)
        .bind(schedule.is_active)
        .bind(schedule.next_run_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn enqueue_post(
        &self,
        entry: NewPostQueueEntry,
    ) -> Result<PostQueueEntry, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let queued = enqueue_post_in_tx(&mut tx, entry).await?;
        tx.commit().await?;
        Ok(queued)
    }

    pub async fn list_queue_for_creator(
        &self,
        creator_id: Uuid,
    ) -> Result<Vec<PostQueueEntry>, sqlx::Error> {
        sqlx::query_as::<_, PostQueueEntry>(
            r#"
            SELECT
                id,
                creator_id,
                post_id,
                scheduled_for,
                queue_position,
                locked_at,
                created_at,
                updated_at
            FROM post_queue_entries
            WHERE creator_id = $1
            ORDER BY scheduled_for ASC, queue_position ASC
            "#,
        )
        .bind(creator_id)
        .fetch_all(&self.pool)
        .await
    }
}

async fn upsert_user_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    creator: &NewAuthenticatedCreator<'_>,
) -> Result<UpsertedUser, sqlx::Error> {
    sqlx::query_as::<_, UpsertedUser>(
        r#"
        INSERT INTO users (sub, email, name, picture_url, email_verified)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (sub) DO UPDATE
        SET email = EXCLUDED.email,
            name = EXCLUDED.name,
            picture_url = EXCLUDED.picture_url,
            email_verified = EXCLUDED.email_verified,
            last_seen_at = NOW()
        RETURNING
            sub,
            email,
            name,
            picture_url,
            email_verified,
            created_at,
            last_seen_at,
            (xmax = 0) AS is_new_registration
        "#,
    )
    .bind(creator.sub)
    .bind(creator.email)
    .bind(creator.name)
    .bind(creator.picture_url)
    .bind(creator.email_verified)
    .fetch_one(&mut **tx)
    .await
}

async fn upsert_creator_for_user_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    creator: &NewAuthenticatedCreator<'_>,
) -> Result<Creator, sqlx::Error> {
    sqlx::query_as::<_, Creator>(
        r#"
        INSERT INTO creators (auth_subject, user_sub, email, display_name, avatar_url)
        VALUES ($1, $1, $2, $3, $4)
        ON CONFLICT (auth_subject) DO UPDATE
        SET user_sub = EXCLUDED.user_sub,
            email = EXCLUDED.email,
            display_name = EXCLUDED.display_name,
            avatar_url = EXCLUDED.avatar_url,
            updated_at = NOW()
        RETURNING
            id,
            auth_subject,
            user_sub,
            email,
            display_name,
            avatar_url,
            created_at,
            updated_at
        "#,
    )
    .bind(creator.sub)
    .bind(creator.email)
    .bind(creator.name)
    .bind(creator.picture_url)
    .fetch_one(&mut **tx)
    .await
}

async fn enqueue_post_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    entry: NewPostQueueEntry,
) -> Result<PostQueueEntry, sqlx::Error> {
    sqlx::query_as::<_, PostQueueEntry>(
        r#"
        INSERT INTO post_queue_entries (creator_id, post_id, scheduled_for, queue_position)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (post_id) DO UPDATE
        SET scheduled_for = EXCLUDED.scheduled_for,
            queue_position = EXCLUDED.queue_position,
            updated_at = NOW()
        RETURNING
            id,
            creator_id,
            post_id,
            scheduled_for,
            queue_position,
            locked_at,
            created_at,
            updated_at
        "#,
    )
    .bind(entry.creator_id)
    .bind(entry.post_id)
    .bind(entry.scheduled_for)
    .bind(entry.queue_position)
    .fetch_one(&mut **tx)
    .await
}
