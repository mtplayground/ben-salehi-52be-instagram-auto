use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::{config::Region, primitives::ByteStream, Client};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    auth::AuthError,
    config::ObjectStorageConfig,
    db::{
        models::{GeneratedPost, MediaAsset, PostStatus},
        repository::NewMediaAsset,
    },
    AppState,
};

const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

#[derive(Clone)]
pub struct ObjectStorageClient {
    config: ObjectStorageConfig,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct StoreFinishedImageRequest {
    image_base64: String,
    mime_type: String,
    source: Option<String>,
    license: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    post_id: Option<Uuid>,
}

#[derive(Debug)]
pub struct StoreFinishedImageInput {
    pub image_base64: String,
    pub mime_type: String,
    pub source: Option<String>,
    pub license: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub post_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct StoreFinishedImageResponse {
    media: MediaAssetPayload,
    post: Option<GeneratedPostPayload>,
}

#[derive(Debug, Serialize)]
pub struct MediaAssetPayload {
    pub id: Uuid,
    pub storage_key: String,
    pub public_url: Option<String>,
    pub source: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<String>,
    pub license: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
struct GeneratedPostPayload {
    id: Uuid,
    media_asset_id: Option<Uuid>,
    status: String,
    updated_at: String,
}

#[derive(Debug)]
struct StoredObject {
    key: String,
    public_url: String,
}

#[derive(Debug)]
struct ValidatedImage {
    bytes: Vec<u8>,
    mime_type: String,
    source: String,
    license: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    post_id: Option<Uuid>,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("object storage is not configured")]
    MissingConfig,
    #[error("{0}")]
    Validation(String),
    #[error("post was not found for this creator")]
    PostNotFound,
    #[error("object storage upload failed: {0}")]
    Upload(String),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/store-image", post(store_finished_image))
}

async fn store_finished_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<StoreFinishedImageRequest>,
) -> Result<Json<StoreFinishedImageResponse>, StorageError> {
    let creator = state.auth.current_creator(&headers).await?;
    let (media, post) =
        store_finished_image_for_creator(&state, creator.creator.id, request.into()).await?;

    Ok(Json(StoreFinishedImageResponse {
        media: media.into(),
        post: post.map(GeneratedPostPayload::from),
    }))
}

pub async fn store_finished_image_for_creator(
    state: &AppState,
    creator_id: Uuid,
    input: StoreFinishedImageInput,
) -> Result<(MediaAsset, Option<GeneratedPost>), StorageError> {
    let image = input.validate()?;

    if let Some(post_id) = image.post_id {
        state
            .repository
            .get_generated_post_for_creator(creator_id, post_id)
            .await?
            .ok_or(StorageError::PostNotFound)?;
    }

    let ValidatedImage {
        bytes,
        mime_type,
        source,
        license,
        width,
        height,
        post_id,
    } = image;

    let storage = object_storage_client(&state)?;
    let stored = storage
        .put_finished_image(creator_id, bytes, mime_type.as_str())
        .await?;

    let media = state
        .repository
        .create_media_asset(NewMediaAsset {
            creator_id,
            storage_key: &stored.key,
            public_url: Some(&stored.public_url),
            source: &source,
            width,
            height,
            mime_type: Some(&mime_type),
            license: license.as_deref(),
        })
        .await?;

    let post = match post_id {
        Some(post_id) => state
            .repository
            .attach_media_asset_to_post(creator_id, post_id, media.id)
            .await?
            .map(Some)
            .ok_or(StorageError::PostNotFound)?,
        None => None,
    };

    Ok((media, post))
}

impl From<StoreFinishedImageRequest> for StoreFinishedImageInput {
    fn from(request: StoreFinishedImageRequest) -> Self {
        Self {
            image_base64: request.image_base64,
            mime_type: request.mime_type,
            source: request.source,
            license: request.license,
            width: request.width,
            height: request.height,
            post_id: request.post_id,
        }
    }
}

impl StoreFinishedImageInput {
    fn validate(self) -> Result<ValidatedImage, StorageError> {
        let mime_type = self.mime_type.trim().to_owned();
        validate_mime_type(&mime_type)?;

        let bytes = STANDARD
            .decode(self.image_base64.trim())
            .map_err(|_| {
                StorageError::Validation("image_base64 must be valid base64.".to_owned())
            })?;
        if bytes.is_empty() {
            return Err(StorageError::Validation(
                "image_base64 must not be empty.".to_owned(),
            ));
        }
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(StorageError::Validation(
                "image_base64 must be 20 MB or smaller.".to_owned(),
            ));
        }

        validate_dimensions(self.width, self.height)?;

        Ok(ValidatedImage {
            bytes,
            mime_type,
            source: normalize_optional(self.source).unwrap_or_else(|| "composited".to_owned()),
            license: normalize_optional(self.license),
            width: self.width,
            height: self.height,
            post_id: self.post_id,
        })
    }
}

impl ObjectStorageClient {
    async fn put_finished_image(
        &self,
        creator_id: Uuid,
        bytes: Vec<u8>,
        mime_type: &str,
    ) -> Result<StoredObject, StorageError> {
        let extension = extension_for_mime_type(mime_type);
        let key = self.storage_key(creator_id, extension);
        let content_length = bytes.len() as i64;

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(ByteStream::from(bytes))
            .content_type(mime_type)
            .content_length(content_length)
            .send()
            .await
            .map_err(|error| StorageError::Upload(error.to_string()))?;

        Ok(StoredObject {
            public_url: self.public_url(&key),
            key,
        })
    }

    fn storage_key(&self, creator_id: Uuid, extension: &str) -> String {
        let prefix = self.config.prefix.trim_matches('/');
        let suffix = format!(
            "creators/{creator_id}/finished-images/{}.{}",
            Uuid::new_v4(),
            extension
        );

        if prefix.is_empty() {
            suffix
        } else {
            format!("{prefix}/{suffix}")
        }
    }

    fn public_url(&self, key: &str) -> String {
        format!(
            "{}/{}/{}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.bucket,
            key
        )
    }
}

impl From<MediaAsset> for MediaAssetPayload {
    fn from(asset: MediaAsset) -> Self {
        Self {
            id: asset.id,
            storage_key: asset.storage_key,
            public_url: asset.public_url,
            source: asset.source,
            width: asset.width,
            height: asset.height,
            mime_type: asset.mime_type,
            license: asset.license,
            created_at: asset.created_at.to_rfc3339(),
        }
    }
}

impl From<GeneratedPost> for GeneratedPostPayload {
    fn from(post: GeneratedPost) -> Self {
        Self {
            id: post.id,
            media_asset_id: post.media_asset_id,
            status: post_status_label(&post.status).to_owned(),
            updated_at: post.updated_at.to_rfc3339(),
        }
    }
}

fn object_storage_client(state: &AppState) -> Result<ObjectStorageClient, StorageError> {
    let config = state
        .config
        .object_storage
        .clone()
        .ok_or(StorageError::MissingConfig)?;
    let credentials = Credentials::new(
        config.access_key_id.clone(),
        config.secret_access_key.clone(),
        None,
        None,
        "object-storage-env",
    );
    let sdk_config = aws_sdk_s3::config::Builder::new()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(config.region.clone()))
        .endpoint_url(config.endpoint.clone())
        .credentials_provider(credentials)
        .force_path_style(true)
        .build();

    Ok(ObjectStorageClient {
        config,
        client: Client::from_conf(sdk_config),
    })
}

fn validate_mime_type(mime_type: &str) -> Result<(), StorageError> {
    match mime_type {
        "image/png" | "image/jpeg" | "image/webp" | "image/svg+xml" => Ok(()),
        _ => Err(StorageError::Validation(
            "mime_type must be image/png, image/jpeg, image/webp, or image/svg+xml.".to_owned(),
        )),
    }
}

fn validate_dimensions(width: Option<i32>, height: Option<i32>) -> Result<(), StorageError> {
    for value in [width, height].into_iter().flatten() {
        if !(1..=4096).contains(&value) {
            return Err(StorageError::Validation(
                "width and height must be between 1 and 4096 when provided.".to_owned(),
            ));
        }
    }

    Ok(())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

fn extension_for_mime_type(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

fn post_status_label(status: &PostStatus) -> &'static str {
    match status {
        PostStatus::Draft => "draft",
        PostStatus::PendingReview => "pending-review",
        PostStatus::Approved => "approved",
        PostStatus::Scheduled => "scheduled",
        PostStatus::Published => "published",
        PostStatus::Failed => "failed",
        PostStatus::Rejected => "rejected",
    }
}

impl IntoResponse for StorageError {
    fn into_response(self) -> Response {
        match self {
            StorageError::Auth(error) => error.into_response(),
            StorageError::Validation(message) => {
                (StatusCode::BAD_REQUEST, Json(error_body(message))).into_response()
            }
            StorageError::MissingConfig => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body("Object storage is not configured.".to_owned())),
            )
                .into_response(),
            StorageError::PostNotFound => (
                StatusCode::NOT_FOUND,
                Json(error_body("Post was not found for this creator.".to_owned())),
            )
                .into_response(),
            StorageError::Upload(error) => {
                tracing::error!(%error, "object storage upload failed");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(error_body("Finished image could not be stored.".to_owned())),
                )
                    .into_response()
            }
            StorageError::Database(error) => {
                tracing::error!(%error, "media storage database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(error_body("Finished image metadata could not be saved.".to_owned())),
                )
                    .into_response()
            }
        }
    }
}

fn error_body(message: String) -> serde_json::Value {
    serde_json::json!({ "error": message })
}
