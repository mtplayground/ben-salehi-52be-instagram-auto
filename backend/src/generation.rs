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
    config::GenerationConfig,
    db::models::ContentSettings,
    AppState,
};

const MAX_HEADER_CHARS: usize = 64;
const MAX_PARAGRAPH_CHARS: usize = 220;
const MAX_CONTEXT_CHARS: usize = 240;
const MAX_CAPTION_CHARS: usize = 2_200;

#[derive(Clone)]
pub struct ImageGenerationClient {
    config: GenerationConfig,
    client: reqwest::Client,
}

#[derive(Clone)]
pub struct CaptionGenerationClient {
    config: GenerationConfig,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct GenerateImageRequest {
    subject_hint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenerateCaptionRequest {
    header_text: String,
    paragraph_text: String,
    post_context: Option<String>,
}

#[derive(Debug, Serialize)]
struct GenerateImageResponse {
    image: GeneratedImagePayload,
}

#[derive(Debug, Serialize)]
struct GenerateCaptionResponse {
    caption: GeneratedCaptionPayload,
}

#[derive(Debug, Serialize)]
pub struct GeneratedImagePayload {
    pub image_base64: String,
    pub mime_type: String,
    pub prompt: String,
    pub revised_prompt: Option<String>,
    pub size: String,
    pub model: String,
    pub byte_length: usize,
}

#[derive(Debug, Serialize)]
pub struct GeneratedCaptionPayload {
    pub caption: String,
    pub model: String,
    pub character_count: usize,
}

#[derive(Debug, Deserialize)]
struct OpenAIImageResponse {
    data: Vec<OpenAIImageData>,
}

#[derive(Debug, Deserialize)]
struct OpenAIImageData {
    b64_json: Option<String>,
    revised_prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatChoice {
    message: OpenAIChatMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatMessage {
    content: Option<String>,
}

#[derive(Debug, Error)]
pub enum GenerationError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("generation is not configured")]
    MissingConfig,
    #[error("content settings are required before generating content")]
    MissingContentSettings,
    #[error("{0}")]
    Validation(String),
    #[error("OpenAI image response did not include base64 image data")]
    MissingImageData,
    #[error("OpenAI image response included invalid base64 image data")]
    InvalidImageData,
    #[error("OpenAI caption response did not include caption text")]
    MissingCaptionData,
    #[error("generation request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/image", post(generate_image))
        .route("/caption", post(generate_caption))
}

pub async fn generate_flat_illustration_for_creator(
    state: &AppState,
    creator_id: Uuid,
    subject_hint: Option<&str>,
) -> Result<GeneratedImagePayload, GenerationError> {
    let settings = state
        .repository
        .get_content_settings(creator_id)
        .await?
        .ok_or(GenerationError::MissingContentSettings)?;
    let client = image_client(state)?;

    client.generate_flat_illustration(&settings, subject_hint).await
}

pub async fn generate_casual_caption_for_creator(
    state: &AppState,
    creator_id: Uuid,
    header_text: &str,
    paragraph_text: &str,
    post_context: Option<&str>,
) -> Result<GeneratedCaptionPayload, GenerationError> {
    let settings = state
        .repository
        .get_content_settings(creator_id)
        .await?
        .ok_or(GenerationError::MissingContentSettings)?;
    let client = caption_client(state)?;
    let input = CaptionInput::validate(header_text, paragraph_text, post_context)?;

    client.generate_caption(&settings, &input).await
}

async fn generate_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GenerateImageRequest>,
) -> Result<Json<GenerateImageResponse>, GenerationError> {
    let creator = state.auth.current_creator(&headers).await?;
    let subject_hint = request.validate_subject_hint()?;
    let image =
        generate_flat_illustration_for_creator(&state, creator.creator.id, subject_hint.as_deref())
            .await?;

    Ok(Json(GenerateImageResponse { image }))
}

async fn generate_caption(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GenerateCaptionRequest>,
) -> Result<Json<GenerateCaptionResponse>, GenerationError> {
    let creator = state.auth.current_creator(&headers).await?;
    let caption = generate_casual_caption_for_creator(
        &state,
        creator.creator.id,
        &request.header_text,
        &request.paragraph_text,
        request.post_context.as_deref(),
    )
    .await?;

    Ok(Json(GenerateCaptionResponse { caption }))
}

impl GenerateImageRequest {
    fn validate_subject_hint(self) -> Result<Option<String>, GenerationError> {
        let Some(value) = self.subject_hint else {
            return Ok(None);
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if trimmed.len() > 240 {
            return Err(GenerationError::Validation(
                "Image subject hint must be 240 characters or fewer.".to_owned(),
            ));
        }

        Ok(Some(trimmed.to_owned()))
    }
}

#[derive(Debug)]
struct CaptionInput {
    header_text: String,
    paragraph_text: String,
    post_context: Option<String>,
}

impl CaptionInput {
    fn validate(
        header_text: &str,
        paragraph_text: &str,
        post_context: Option<&str>,
    ) -> Result<Self, GenerationError> {
        let header_text = normalize_text(header_text);
        let paragraph_text = normalize_text(paragraph_text);
        let post_context = post_context.and_then(normalize_optional_text);

        if header_text.len() < 3 {
            return Err(GenerationError::Validation(
                "Header text must be at least 3 characters.".to_owned(),
            ));
        }
        if header_text.len() > MAX_HEADER_CHARS {
            return Err(GenerationError::Validation(format!(
                "Header text must be {MAX_HEADER_CHARS} characters or fewer."
            )));
        }
        if paragraph_text.len() < 12 {
            return Err(GenerationError::Validation(
                "Paragraph text must be at least 12 characters.".to_owned(),
            ));
        }
        if paragraph_text.len() > MAX_PARAGRAPH_CHARS {
            return Err(GenerationError::Validation(format!(
                "Paragraph text must be {MAX_PARAGRAPH_CHARS} characters or fewer."
            )));
        }
        if post_context
            .as_ref()
            .is_some_and(|value| value.len() > MAX_CONTEXT_CHARS)
        {
            return Err(GenerationError::Validation(format!(
                "Post context must be {MAX_CONTEXT_CHARS} characters or fewer."
            )));
        }

        Ok(Self {
            header_text,
            paragraph_text,
            post_context,
        })
    }
}

impl ImageGenerationClient {
    async fn generate_flat_illustration(
        &self,
        settings: &ContentSettings,
        subject_hint: Option<&str>,
    ) -> Result<GeneratedImagePayload, GenerationError> {
        let prompt = illustration_prompt(settings, subject_hint);
        let body = serde_json::json!({
            "model": &self.config.image_model,
            "prompt": &prompt,
            "n": 1,
            "size": &self.config.image_size,
            "quality": &self.config.image_quality,
            "output_format": &self.config.image_format,
        });

        let response = self
            .client
            .post(&self.config.image_generation_url)
            .bearer_auth(&self.config.openai_api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<OpenAIImageResponse>()
            .await?;

        let first = response
            .data
            .into_iter()
            .next()
            .ok_or(GenerationError::MissingImageData)?;
        let image_base64 = first.b64_json.ok_or(GenerationError::MissingImageData)?;
        let byte_length = STANDARD
            .decode(&image_base64)
            .map_err(|_| GenerationError::InvalidImageData)?
            .len();

        Ok(GeneratedImagePayload {
            image_base64,
            mime_type: format!("image/{}", self.config.image_format),
            prompt,
            revised_prompt: first.revised_prompt,
            size: self.config.image_size.clone(),
            model: self.config.image_model.clone(),
            byte_length,
        })
    }
}

impl CaptionGenerationClient {
    async fn generate_caption(
        &self,
        settings: &ContentSettings,
        input: &CaptionInput,
    ) -> Result<GeneratedCaptionPayload, GenerationError> {
        let body = serde_json::json!({
            "model": &self.config.caption_model,
            "messages": [
                {
                    "role": "system",
                    "content": caption_system_prompt(),
                },
                {
                    "role": "user",
                    "content": caption_user_prompt(settings, input),
                }
            ],
            "temperature": 0.8,
            "max_completion_tokens": 180,
        });

        let response = self
            .client
            .post(&self.config.caption_generation_url)
            .bearer_auth(&self.config.openai_api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<OpenAIChatResponse>()
            .await?;

        let raw_caption = response
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or(GenerationError::MissingCaptionData)?;
        let caption = clean_caption(&raw_caption)?;

        Ok(GeneratedCaptionPayload {
            character_count: caption.chars().count(),
            caption,
            model: self.config.caption_model.clone(),
        })
    }
}

fn image_client(state: &AppState) -> Result<ImageGenerationClient, GenerationError> {
    let config = state
        .config
        .generation
        .clone()
        .ok_or(GenerationError::MissingConfig)?;

    Ok(ImageGenerationClient {
        config,
        client: reqwest::Client::new(),
    })
}

fn caption_client(state: &AppState) -> Result<CaptionGenerationClient, GenerationError> {
    let config = state
        .config
        .generation
        .clone()
        .ok_or(GenerationError::MissingConfig)?;

    Ok(CaptionGenerationClient {
        config,
        client: reqwest::Client::new(),
    })
}

fn illustration_prompt(settings: &ContentSettings, subject_hint: Option<&str>) -> String {
    let subject = subject_hint.unwrap_or(settings.theme_topic.as_str());
    format!(
        "Create one simple flat vector-style illustration for an Instagram post. \
Use a clean minimal composition, solid shapes, soft contrast, limited color palette, \
no photorealism, no tiny details, no text, no letters, no logos, no watermark. \
Creator theme/topic: {}. Creator style notes: {}. Specific subject for this post: {}.",
        settings.theme_topic, settings.style_notes, subject
    )
}

fn caption_system_prompt() -> &'static str {
    "You write Instagram captions for a creator. Write in first person, casual, friendly, \
conversational language, like the creator talking directly to followers. Keep it aligned \
with the supplied on-image header and paragraph. Do not sound corporate. Do not mention \
that you are an AI. Do not use markdown. Return only the caption text."
}

fn caption_user_prompt(settings: &ContentSettings, input: &CaptionInput) -> String {
    let context = input.post_context.as_deref().unwrap_or("No extra context.");
    format!(
        "Creator theme/topic: {}\nCreator style notes: {}\nOn-image header: {}\n\
On-image paragraph: {}\nPost context: {}\n\nWrite one caption under 900 characters. \
Use one or two short paragraphs, optional light emojis only if they fit the style, and \
zero to four relevant hashtags at the end.",
        settings.theme_topic,
        settings.style_notes,
        input.header_text,
        input.paragraph_text,
        context
    )
}

fn clean_caption(raw_caption: &str) -> Result<String, GenerationError> {
    let caption = raw_caption
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<&str>>()
        .join("\n\n");
    let caption = caption.trim_matches('"').trim().to_owned();

    if caption.is_empty() {
        return Err(GenerationError::MissingCaptionData);
    }

    if caption.chars().count() <= MAX_CAPTION_CHARS {
        return Ok(caption);
    }

    let mut truncated = caption
        .chars()
        .take(MAX_CAPTION_CHARS.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    Ok(truncated)
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn normalize_optional_text(value: &str) -> Option<String> {
    let trimmed = normalize_text(value);
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

impl IntoResponse for GenerationError {
    fn into_response(self) -> Response {
        match self {
            GenerationError::Auth(error) => error.into_response(),
            GenerationError::Validation(message) => {
                (StatusCode::BAD_REQUEST, Json(error_body(message))).into_response()
            }
            GenerationError::MissingContentSettings => (
                StatusCode::BAD_REQUEST,
                Json(error_body(
                    "Content settings are required before generating content.".to_owned(),
                )),
            )
                .into_response(),
            GenerationError::MissingConfig => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body("Generation is not configured.".to_owned())),
            )
                .into_response(),
            error @ (GenerationError::MissingImageData | GenerationError::InvalidImageData) => {
                tracing::error!(%error, "OpenAI image generation returned invalid data");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(error_body("Image generation returned invalid data.".to_owned())),
                )
                    .into_response()
            }
            GenerationError::MissingCaptionData => {
                tracing::error!("OpenAI caption generation returned empty caption data");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(error_body("Caption generation returned invalid data.".to_owned())),
                )
                    .into_response()
            }
            GenerationError::Request(error) => {
                tracing::error!(%error, "OpenAI generation request failed");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(error_body("Generation failed.".to_owned())),
                )
                    .into_response()
            }
            GenerationError::Database(error) => {
                tracing::error!(%error, "generation database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(error_body("Generation could not load settings.".to_owned())),
                )
                    .into_response()
            }
        }
    }
}

fn error_body(message: String) -> serde_json::Value {
    serde_json::json!({ "error": message })
}
