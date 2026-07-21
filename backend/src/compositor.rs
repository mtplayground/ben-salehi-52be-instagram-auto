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

use crate::{auth::AuthError, AppState};

const CANVAS_SIZE: u32 = 1080;
const MAX_IMAGE_BYTES: usize = 12 * 1024 * 1024;
const MAX_HEADER_CHARS: usize = 64;
const MAX_PARAGRAPH_CHARS: usize = 220;

#[derive(Debug, Deserialize)]
struct OverlayTextRequest {
    image_base64: Option<String>,
    image_url: Option<String>,
    mime_type: Option<String>,
    header_text: String,
    paragraph_text: String,
}

#[derive(Debug, Serialize)]
struct OverlayTextResponse {
    image: CompositedImagePayload,
}

#[derive(Debug, Serialize)]
pub struct CompositedImagePayload {
    pub image_base64: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub header_text: String,
    pub paragraph_text: String,
    pub byte_length: usize,
}

#[derive(Debug)]
pub struct OverlayTextInput {
    pub image_source: ImageSource,
    pub header_text: String,
    pub paragraph_text: String,
}

#[derive(Debug)]
pub enum ImageSource {
    DataUri { mime_type: String, image_base64: String },
    Url(String),
}

#[derive(Debug, Error)]
pub enum CompositorError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("{0}")]
    Validation(String),
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/overlay", post(overlay_text))
}

pub fn compose_text_overlay(
    input: OverlayTextInput,
) -> Result<CompositedImagePayload, CompositorError> {
    let header_lines = wrap_text(&input.header_text, 26, 1);
    let paragraph_lines = wrap_text(&input.paragraph_text, 42, 4);
    let image_href = match input.image_source {
        ImageSource::DataUri {
            mime_type,
            image_base64,
        } => format!("data:{};base64,{}", mime_type, image_base64),
        ImageSource::Url(url) => url,
    };

    let svg = render_svg(&image_href, &header_lines, &paragraph_lines);
    let byte_length = svg.len();

    Ok(CompositedImagePayload {
        image_base64: STANDARD.encode(svg.as_bytes()),
        mime_type: "image/svg+xml".to_owned(),
        width: CANVAS_SIZE,
        height: CANVAS_SIZE,
        header_text: input.header_text,
        paragraph_text: input.paragraph_text,
        byte_length,
    })
}

async fn overlay_text(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OverlayTextRequest>,
) -> Result<Json<OverlayTextResponse>, CompositorError> {
    state.auth.current_creator(&headers).await?;
    let image = compose_text_overlay(request.validate()?)?;

    Ok(Json(OverlayTextResponse { image }))
}

impl OverlayTextRequest {
    fn validate(self) -> Result<OverlayTextInput, CompositorError> {
        let header_text = normalize_text(&self.header_text);
        let paragraph_text = normalize_text(&self.paragraph_text);

        if header_text.len() < 3 {
            return Err(CompositorError::Validation(
                "Header text must be at least 3 characters.".to_owned(),
            ));
        }
        if header_text.len() > MAX_HEADER_CHARS {
            return Err(CompositorError::Validation(format!(
                "Header text must be {MAX_HEADER_CHARS} characters or fewer."
            )));
        }
        if paragraph_text.len() < 12 {
            return Err(CompositorError::Validation(
                "Paragraph text must be at least 12 characters.".to_owned(),
            ));
        }
        if paragraph_text.len() > MAX_PARAGRAPH_CHARS {
            return Err(CompositorError::Validation(format!(
                "Paragraph text must be {MAX_PARAGRAPH_CHARS} characters or fewer."
            )));
        }

        let image_source = match (self.image_base64, self.image_url) {
            (Some(image_base64), None) => {
                let mime_type = self
                    .mime_type
                    .unwrap_or_else(|| "image/png".to_owned())
                    .trim()
                    .to_owned();
                validate_mime_type(&mime_type)?;
                validate_base64_image(&image_base64)?;
                ImageSource::DataUri {
                    mime_type,
                    image_base64,
                }
            }
            (None, Some(image_url)) => {
                let image_url = image_url.trim().to_owned();
                validate_image_url(&image_url)?;
                ImageSource::Url(image_url)
            }
            (Some(_), Some(_)) => {
                return Err(CompositorError::Validation(
                    "Provide either image_base64 or image_url, not both.".to_owned(),
                ));
            }
            (None, None) => {
                return Err(CompositorError::Validation(
                    "An image_base64 or image_url value is required.".to_owned(),
                ));
            }
        };

        Ok(OverlayTextInput {
            image_source,
            header_text,
            paragraph_text,
        })
    }
}

fn render_svg(image_href: &str, header_lines: &[String], paragraph_lines: &[String]) -> String {
    let escaped_href = escape_xml(image_href);
    let paragraph_line_count = paragraph_lines.len().max(1) as u32;
    let overlay_height = 244 + paragraph_line_count.saturating_sub(1) * 54;
    let overlay_y = CANVAS_SIZE - overlay_height - 84;
    let header_y = overlay_y + 100;
    let paragraph_y = header_y + 100;
    let header = text_tspans(header_lines, 160, 92);
    let paragraph = text_tspans(paragraph_lines, 160, 54);

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}" role="img">
<defs>
<linearGradient id="image-vignette" x1="0" y1="0" x2="0" y2="1">
<stop offset="0%" stop-color="#000" stop-opacity="0.05"/>
<stop offset="52%" stop-color="#000" stop-opacity="0.12"/>
<stop offset="100%" stop-color="#000" stop-opacity="0.68"/>
</linearGradient>
<filter id="text-shadow" x="-20%" y="-20%" width="140%" height="140%">
<feDropShadow dx="0" dy="6" stdDeviation="8" flood-color="#000" flood-opacity="0.38"/>
</filter>
</defs>
<rect width="{size}" height="{size}" fill="#111827"/>
<image href="{href}" width="{size}" height="{size}" preserveAspectRatio="xMidYMid slice"/>
<rect width="{size}" height="{size}" fill="url(#image-vignette)"/>
<rect x="84" y="{overlay_y}" width="912" height="{overlay_height}" rx="34" fill="#111827" opacity="0.78"/>
<rect x="86" y="{overlay_y}" width="908" height="{overlay_height}" rx="32" fill="none" stroke="#ffffff" stroke-opacity="0.12" stroke-width="2"/>
<text x="126" y="{header_y}" fill="#ffffff" font-family="Inter, Arial, sans-serif" font-size="74" font-weight="800" letter-spacing="0" filter="url(#text-shadow)">{header}</text>
<text x="126" y="{paragraph_y}" fill="#f8fafc" font-family="Inter, Arial, sans-serif" font-size="38" font-weight="500" letter-spacing="0" opacity="0.96">{paragraph}</text>
</svg>"##,
        size = CANVAS_SIZE,
        href = escaped_href,
        overlay_y = overlay_y,
        overlay_height = overlay_height,
        header_y = header_y,
        paragraph_y = paragraph_y,
        header = header,
        paragraph = paragraph
    )
}

fn text_tspans(lines: &[String], x: u32, line_height: u32) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let escaped = escape_xml(line);
            let dy = if index == 0 {
                0
            } else {
                line_height
            };
            format!(r#"<tspan x="{x}" dy="{dy}">{escaped}</tspan>"#)
        })
        .collect::<Vec<String>>()
        .join("")
}

fn wrap_text(value: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let words = value.split_whitespace().collect::<Vec<&str>>();
    let mut lines = Vec::<String>::new();
    let mut current = String::new();

    for word in words {
        let next_len = if current.is_empty() {
            word.len()
        } else {
            current.len() + 1 + word.len()
        };

        if next_len <= max_chars {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            continue;
        }

        if !current.is_empty() {
            lines.push(current);
            current = String::new();
        }

        if word.len() > max_chars {
            lines.push(word.chars().take(max_chars).collect());
        } else {
            current.push_str(word);
        }

        if lines.len() == max_lines {
            return lines;
        }
    }

    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }

    lines
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn validate_mime_type(mime_type: &str) -> Result<(), CompositorError> {
    match mime_type {
        "image/png" | "image/jpeg" | "image/webp" | "image/svg+xml" => Ok(()),
        _ => Err(CompositorError::Validation(
            "Image MIME type must be image/png, image/jpeg, image/webp, or image/svg+xml."
                .to_owned(),
        )),
    }
}

fn validate_base64_image(image_base64: &str) -> Result<(), CompositorError> {
    let bytes = STANDARD.decode(image_base64).map_err(|_| {
        CompositorError::Validation("image_base64 must contain valid base64 data.".to_owned())
    })?;
    if bytes.is_empty() {
        return Err(CompositorError::Validation(
            "image_base64 must not be empty.".to_owned(),
        ));
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(CompositorError::Validation(
            "image_base64 must be 12 MB or smaller.".to_owned(),
        ));
    }

    Ok(())
}

fn validate_image_url(image_url: &str) -> Result<(), CompositorError> {
    if image_url.starts_with("https://") || image_url.starts_with("http://") {
        return Ok(());
    }

    Err(CompositorError::Validation(
        "image_url must start with http:// or https://.".to_owned(),
    ))
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

impl IntoResponse for CompositorError {
    fn into_response(self) -> Response {
        match self {
            CompositorError::Auth(error) => error.into_response(),
            CompositorError::Validation(message) => {
                (StatusCode::BAD_REQUEST, Json(error_body(message))).into_response()
            }
        }
    }
}

fn error_body(message: String) -> serde_json::Value {
    serde_json::json!({ "error": message })
}
