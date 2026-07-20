use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    auth::AuthError,
    config::InstagramConfig,
    credentials::{CredentialCipher, CredentialError},
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
    cipher: CredentialCipher,
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
    access_token: String,
    user_id: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct LongLivedTokenResponse {
    access_token: String,
    expires_in: i64,
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
    token_expires_at: Option<String>,
    connected_at: String,
    disconnected_at: Option<String>,
}

#[derive(Debug)]
pub struct InstagramCredential {
    pub account_id: Uuid,
    pub instagram_user_id: String,
    pub access_token: String,
    pub token_expires_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum InstagramError {
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
    #[error("Instagram credential storage is invalid: {0}")]
    Credential(#[from] CredentialError),
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
        .route("/refresh", post(refresh))
        .route("/disconnect", post(disconnect))
}

pub async fn get_valid_instagram_credential(
    state: &AppState,
    creator_id: Uuid,
) -> Result<Option<InstagramCredential>, InstagramError> {
    let Some(account) = state
        .repository
        .get_instagram_account_for_creator(creator_id)
        .await?
    else {
        return Ok(None);
    };

    let account = refresh_account_if_needed(state, account).await?;
    if account.connection_status != "connected" {
        return Ok(None);
    }

    let Some(encrypted_token) = account.access_token_ciphertext.clone() else {
        mark_reconnect_needed(state, account, "Instagram authorization is missing.").await?;
        return Ok(None);
    };
    let Some(token_expires_at) = account.token_expires_at.clone() else {
        mark_reconnect_needed(state, account, "Instagram authorization is missing.").await?;
        return Ok(None);
    };

    let client = instagram_client(state)?;
    match client.cipher.decrypt(&encrypted_token) {
        Ok(access_token) => Ok(Some(InstagramCredential {
            account_id: account.id,
            instagram_user_id: account.instagram_user_id,
            access_token,
            token_expires_at,
        })),
        Err(error) => {
            tracing::error!(%error, account_id = %account.id, "Instagram credential could not be decrypted");
            mark_reconnect_needed(
                state,
                account,
                "Instagram authorization could not be read. Reconnect the account.",
            )
            .await?;
            Ok(None)
        }
    }
}

async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<InstagramStatusResponse>, InstagramError> {
    let creator = state.auth.current_creator(&headers).await?;
    let account = match state
        .repository
        .get_instagram_account_for_creator(creator.creator.id)
        .await?
    {
        Some(account) => Some(refresh_account_if_needed(&state, account).await?),
        None => None,
    }
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
    let long_lived_token = client.exchange_long_lived_token(&token.access_token).await?;
    let encrypted_access_token = client.cipher.encrypt(&long_lived_token.access_token)?;
    let token_expires_at = expires_at(long_lived_token.expires_in)?;
    state
        .repository
        .upsert_instagram_account(NewInstagramAccount {
            creator_id: creator.creator.id,
            instagram_user_id: &instagram_user_id,
            username: None,
            access_token_ciphertext: Some(&encrypted_access_token),
            refresh_token_ciphertext: None,
            token_expires_at: Some(token_expires_at),
        })
        .await?;

    Ok(Redirect::to("/connections?instagram=connected"))
}

async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<InstagramStatusResponse>, InstagramError> {
    let creator = state.auth.current_creator(&headers).await?;
    let account = match state
        .repository
        .get_instagram_account_for_creator(creator.creator.id)
        .await?
    {
        Some(account) => Some(refresh_instagram_account(&state, account).await?),
        None => None,
    };

    Ok(Json(InstagramStatusResponse {
        account: account.map(InstagramAccountPayload::from),
    }))
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

    async fn exchange_long_lived_token(
        &self,
        short_lived_access_token: &str,
    ) -> Result<LongLivedTokenResponse, reqwest::Error> {
        self.client
            .get(&self.config.long_lived_token_url)
            .query(&[
                ("grant_type", "ig_exchange_token"),
                ("client_secret", self.config.client_secret.as_str()),
                ("access_token", short_lived_access_token),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<LongLivedTokenResponse>()
            .await
    }

    async fn refresh_long_lived_token(
        &self,
        long_lived_access_token: &str,
    ) -> Result<LongLivedTokenResponse, reqwest::Error> {
        self.client
            .get(&self.config.refresh_token_url)
            .query(&[
                ("grant_type", "ig_refresh_token"),
                ("access_token", long_lived_access_token),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<LongLivedTokenResponse>()
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
            token_expires_at: account.token_expires_at.map(|value| value.to_rfc3339()),
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
            InstagramError::Credential(error) => {
                tracing::error!(%error, "Instagram credential encryption failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(error_body("Instagram credentials could not be processed.".to_owned())),
                )
                    .into_response()
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
    let cipher = CredentialCipher::from_base64_key(&config.credential_encryption_key)?;

    Ok(InstagramOAuthClient {
        config,
        client: reqwest::Client::new(),
        cipher,
    })
}

async fn refresh_account_if_needed(
    state: &AppState,
    account: InstagramAccount,
) -> Result<InstagramAccount, InstagramError> {
    if account.connection_status != "connected" {
        return Ok(account);
    }

    let Some(token_expires_at) = account.token_expires_at.clone() else {
        return mark_reconnect_needed(state, account, "Instagram authorization is missing.").await;
    };

    if token_expires_at > Utc::now() + Duration::days(7) {
        return Ok(account);
    }

    refresh_instagram_account(state, account).await
}

async fn refresh_instagram_account(
    state: &AppState,
    account: InstagramAccount,
) -> Result<InstagramAccount, InstagramError> {
    if account.connection_status == "disconnected" {
        return Ok(account);
    }

    let Some(encrypted_token) = account.access_token_ciphertext.clone() else {
        return mark_reconnect_needed(state, account, "Instagram authorization is missing.").await;
    };

    let client = instagram_client(state)?;
    let access_token = match client.cipher.decrypt(&encrypted_token) {
        Ok(token) => token,
        Err(error) => {
            tracing::error!(%error, account_id = %account.id, "Instagram credential could not be decrypted");
            return mark_reconnect_needed(
                state,
                account,
                "Instagram authorization could not be read. Reconnect the account.",
            )
            .await;
        }
    };

    match client.refresh_long_lived_token(&access_token).await {
        Ok(token) => {
            let encrypted_access_token = client.cipher.encrypt(&token.access_token)?;
            let token_expires_at = expires_at(token.expires_in)?;
            state
                .repository
                .update_instagram_account_token(account.id, &encrypted_access_token, token_expires_at)
                .await
                .map_err(InstagramError::Database)
        }
        Err(error) => {
            tracing::warn!(
                %error,
                account_id = %account.id,
                "Instagram token refresh failed; marking account reconnect-needed"
            );
            mark_reconnect_needed(
                state,
                account,
                "Instagram authorization expired or was revoked. Reconnect the account.",
            )
            .await
        }
    }
}

async fn mark_reconnect_needed(
    state: &AppState,
    account: InstagramAccount,
    reason: &str,
) -> Result<InstagramAccount, InstagramError> {
    state
        .repository
        .mark_instagram_account_reconnect_needed(account.id, reason)
        .await
        .map_err(InstagramError::Database)
}

fn expires_at(expires_in_seconds: i64) -> Result<DateTime<Utc>, InstagramError> {
    let seconds = expires_in_seconds.max(0);
    Utc::now()
        .checked_add_signed(Duration::seconds(seconds))
        .ok_or(InstagramError::InvalidTokenResponse)
}

fn error_body(message: String) -> serde_json::Value {
    serde_json::json!({ "error": message })
}
