use chrono::{Duration, Utc};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    auth::AuthError,
    config::InstagramConfig,
    db::{
        models::InstagramAccount,
        repository::{NewInstagramAccount, NewInstagramOAuthState},
    },
    AppState,
};

#[derive(Clone)]
struct InstagramOAuthClient {
    config: InstagramConfig,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_reason: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(rename = "access_token")]
    _access_token: String,
    user_id: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct InstagramStatusResponse {
    account: Option<InstagramAccountPayload>,
}

#[derive(Debug, Serialize)]
struct InstagramAccountPayload {
    id: Uuid,
    instagram_user_id: String,
    username: Option<String>,
    connection_status: String,
    reconnect_reason: Option<String>,
    connected_at: String,
    disconnected_at: Option<String>,
}

#[derive(Debug, Error)]
enum InstagramError {
    #[error("{0}")]
    Auth(#[from] AuthError),
    #[error("Instagram configuration must be set")]
    MissingConfig,
    #[error("Instagram authorization did not include a code")]
    MissingCode,
    #[error("Instagram authorization state is invalid or expired")]
    InvalidState,
    #[error("Instagram token response was missing an account id")]
    InvalidTokenResponse,
    #[error("Instagram token exchange failed: {0}")]
    TokenExchange(#[from] reqwest::Error),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/status", get(status))
        .route("/connect", get(connect))
        .route("/callback", get(callback))
        .route("/disconnect", post(disconnect))
}

async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<InstagramStatusResponse>, InstagramError> {
    let creator = state.auth.current_creator(&headers).await?;
    let account = state
        .repository
        .get_instagram_account_for_creator(creator.creator.id)
        .await?
        .map(InstagramAccountPayload::from);

    Ok(Json(InstagramStatusResponse { account }))
}

async fn connect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Redirect, InstagramError> {
    let creator = state.auth.current_creator(&headers).await?;
    let client = instagram_client(&state)?;
    let state_value = Uuid::new_v4().to_string();
    state
        .repository
        .create_instagram_oauth_state(NewInstagramOAuthState {
            creator_id: creator.creator.id,
            state: &state_value,
            return_path: "/connections",
            expires_at: Utc::now() + Duration::minutes(10),
        })
        .await?;

    Ok(Redirect::temporary(&client.authorization_url(&state_value)))
}

async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Result<Redirect, InstagramError> {
    if let Some(error) = query.error.as_deref() {
        tracing::warn!(
            error,
            reason = query.error_reason.as_deref(),
            description = query.error_description.as_deref(),
            "Instagram authorization was not completed"
        );
        return Ok(Redirect::to("/connections?instagram=denied"));
    }

    let creator = state.auth.current_creator(&headers).await?;
    let state_value = query.state.as_deref().ok_or(InstagramError::InvalidState)?;
    state
        .repository
        .consume_instagram_oauth_state(creator.creator.id, state_value)
        .await?
        .ok_or(InstagramError::InvalidState)?;

    let code = query.code.as_deref().ok_or(InstagramError::MissingCode)?;
    let client = instagram_client(&state)?;
    let token = client.exchange_code(code).await?;
    let instagram_user_id = token.user_id_string().ok_or(InstagramError::InvalidTokenResponse)?;
    state
        .repository
        .upsert_instagram_account(NewInstagramAccount {
            creator_id: creator.creator.id,
            instagram_user_id: &instagram_user_id,
            username: None,
            access_token_ciphertext: None,
            refresh_token_ciphertext: None,
            token_expires_at: None,
        })
        .await?;

    Ok(Redirect::to("/connections?instagram=connected"))
}

async fn disconnect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<InstagramStatusResponse>, InstagramError> {
    let creator = state.auth.current_creator(&headers).await?;
    let account = match state
        .repository
        .disconnect_instagram_accounts(creator.creator.id)
        .await?
    {
        Some(account) => Some(account),
        None => state
            .repository
            .get_instagram_account_for_creator(creator.creator.id)
            .await?,
    };

    Ok(Json(InstagramStatusResponse {
        account: account.map(InstagramAccountPayload::from),
    }))
}

impl InstagramOAuthClient {
    fn authorization_url(&self, state: &str) -> String {
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            self.config.auth_url,
            urlencoding::encode(&self.config.client_id),
            urlencoding::encode(&self.config.redirect_uri),
            urlencoding::encode(&self.config.scopes),
            urlencoding::encode(state)
        )
    }

    async fn exchange_code(&self, code: &str) -> Result<TokenResponse, reqwest::Error> {
        self.client
            .post(&self.config.token_url)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
                ("grant_type", "authorization_code"),
                ("redirect_uri", self.config.redirect_uri.as_str()),
                ("code", code),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<TokenResponse>()
            .await
    }
}

impl TokenResponse {
    fn user_id_string(&self) -> Option<String> {
        if let Some(value) = self.user_id.as_str() {
            return Some(value.to_owned());
        }

        if let Some(value) = self.user_id.as_u64() {
            return Some(value.to_string());
        }

        self.user_id.as_i64().map(|value| value.to_string())
    }
}

impl From<InstagramAccount> for InstagramAccountPayload {
    fn from(account: InstagramAccount) -> Self {
        Self {
            id: account.id,
            instagram_user_id: account.instagram_user_id,
            username: account.username,
            connection_status: account.connection_status,
            reconnect_reason: account.reconnect_reason,
            connected_at: account.connected_at.to_rfc3339(),
            disconnected_at: account.disconnected_at.map(|value| value.to_rfc3339()),
        }
    }
}

impl IntoResponse for InstagramError {
    fn into_response(self) -> Response {
        match self {
            InstagramError::Auth(error) => error.into_response(),
            InstagramError::MissingCode
            | InstagramError::InvalidState
            | InstagramError::InvalidTokenResponse => {
                (StatusCode::BAD_REQUEST, Json(error_body(self.to_string()))).into_response()
            }
            InstagramError::MissingConfig => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body("Instagram connection is not configured.".to_owned())),
            )
                .into_response(),
            InstagramError::TokenExchange(error) => {
                tracing::error!(%error, "Instagram token exchange failed");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(error_body("Instagram authorization could not be completed.".to_owned())),
                )
                    .into_response()
            }
            InstagramError::Database(error) => {
                tracing::error!(%error, "Instagram connection database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(error_body("Instagram connection could not be updated.".to_owned())),
                )
                    .into_response()
            }
        }
    }
}

fn instagram_client(state: &AppState) -> Result<InstagramOAuthClient, InstagramError> {
    let config = state
        .config
        .instagram
        .clone()
        .ok_or(InstagramError::MissingConfig)?;

    Ok(InstagramOAuthClient {
        config,
        client: reqwest::Client::new(),
    })
}

fn error_body(message: String) -> serde_json::Value {
    serde_json::json!({ "error": message })
}
