use std::sync::Arc;

use axum::{
    extract::State,
    http::{
        header::{COOKIE, HOST, SET_COOKIE},
        HeaderMap, StatusCode,
    },
    response::{AppendHeaders, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    config::{AppConfig, AuthConfig},
    db::repository::{AuthenticatedCreator, CoreRepository, NewAuthenticatedCreator},
    AppState,
};

#[derive(Clone)]
pub struct AuthService {
    config: AuthConfig,
    client: reqwest::Client,
    repository: CoreRepository,
    app_config: Arc<AppConfig>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionUser {
    pub sub: String,
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub picture: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreatorIdentity {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthMeResponse {
    pub authenticated: bool,
    pub user: SessionUser,
    pub creator: CreatorIdentity,
    pub is_new_registration: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub authenticated: bool,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("MCTAI auth configuration must be set")]
    MissingConfig,
    #[error("missing mctai_session cookie")]
    MissingSession,
    #[error("invalid session")]
    InvalidSession,
    #[error("session is missing required claim {0}")]
    MissingClaim(&'static str),
    #[error("auth key fetch failed: {0}")]
    KeyFetch(#[from] reqwest::Error),
    #[error("auth key not found")]
    KeyNotFound,
    #[error("auth key is invalid")]
    InvalidKey,
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

impl AuthService {
    pub fn new(app_config: Arc<AppConfig>, repository: CoreRepository) -> Result<Self, AuthError> {
        let config = app_config.auth.clone().ok_or(AuthError::MissingConfig)?;

        Ok(Self {
            config,
            client: reqwest::Client::new(),
            repository,
            app_config,
        })
    }

    pub fn login_url(&self, headers: &HeaderMap) -> String {
        let return_to = format!("{}/", public_origin(headers, &self.app_config));
        format!(
            "{}/login?app_token={}&return_to={}",
            self.config.url,
            urlencoding::encode(&self.config.app_token),
            urlencoding::encode(&return_to)
        )
    }

    pub async fn current_creator(
        &self,
        headers: &HeaderMap,
    ) -> Result<AuthenticatedCreator, AuthError> {
        let claims = self.verify_session(headers).await?;

        self.repository
            .upsert_authenticated_creator(NewAuthenticatedCreator {
                sub: &claims.sub,
                email: &claims.email,
                email_verified: claims.email_verified,
                name: claims.name.as_deref(),
                picture_url: claims.picture.as_deref(),
            })
            .await
            .map_err(AuthError::Database)
    }

    async fn verify_session(&self, headers: &HeaderMap) -> Result<SessionUser, AuthError> {
        let token = session_cookie(headers).ok_or(AuthError::MissingSession)?;
        let header = decode_header(&token).map_err(|_| AuthError::InvalidSession)?;
        let kid = header.kid.ok_or(AuthError::InvalidSession)?;
        let decoding_key = self.decoding_key(&kid).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.config.app_token.as_str()]);
        validation.set_issuer(&[self.config.url.as_str()]);

        let token_data =
            decode::<Value>(&token, &decoding_key, &validation).map_err(|_| AuthError::InvalidSession)?;

        SessionUser::from_claims(token_data.claims)
    }

    async fn decoding_key(&self, kid: &str) -> Result<DecodingKey, AuthError> {
        let jwks = self
            .client
            .get(&self.config.jwks_url)
            .send()
            .await?
            .error_for_status()?
            .json::<Jwks>()
            .await?;

        let jwk = jwks
            .keys
            .into_iter()
            .find(|key| key.kid == kid)
            .ok_or(AuthError::KeyNotFound)?;

        DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|_| AuthError::InvalidKey)
    }
}

impl SessionUser {
    fn from_claims(claims: Value) -> Result<Self, AuthError> {
        let sub = string_claim(&claims, "sub").ok_or(AuthError::MissingClaim("sub"))?;
        let email = string_claim(&claims, "email").ok_or(AuthError::MissingClaim("email"))?;
        let email_verified = claims
            .get("email_verified")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let name = string_claim(&claims, "name");
        let picture = string_claim(&claims, "picture");

        Ok(Self {
            sub,
            email,
            email_verified,
            name,
            picture,
        })
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match &self {
            AuthError::MissingSession
            | AuthError::InvalidSession
            | AuthError::MissingClaim(_)
            | AuthError::KeyNotFound => StatusCode::UNAUTHORIZED,
            AuthError::MissingConfig
            | AuthError::KeyFetch(_)
            | AuthError::InvalidKey
            | AuthError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(serde_json::json!({
            "error": self.to_string(),
        }));

        (status, body).into_response()
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

async fn login(State(state): State<AppState>, headers: HeaderMap) -> Redirect {
    Redirect::temporary(&state.auth.login_url(&headers))
}

async fn logout() -> impl IntoResponse {
    (
        AppendHeaders([
            (
                SET_COOKIE,
                "mctai_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax; Secure",
            ),
            (
                SET_COOKIE,
                "mctai_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax",
            ),
        ]),
        Json(LogoutResponse {
            authenticated: false,
        }),
    )
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthMeResponse>, AuthError> {
    let authenticated = state.auth.current_creator(&headers).await?;
    let user = SessionUser {
        sub: authenticated.user.sub,
        email: authenticated.user.email,
        email_verified: authenticated.user.email_verified,
        name: authenticated.user.name,
        picture: authenticated.user.picture_url,
    };

    let message = if authenticated.is_new_registration {
        "Registration complete!".to_owned()
    } else {
        match user.name.as_deref() {
            Some(name) => format!("Welcome back, {name}!"),
            None => "Welcome back!".to_owned(),
        }
    };

    Ok(Json(AuthMeResponse {
        authenticated: true,
        creator: CreatorIdentity {
            id: authenticated.creator.id,
            email: authenticated.creator.email,
            display_name: authenticated.creator.display_name,
            avatar_url: authenticated.creator.avatar_url,
        },
        user,
        is_new_registration: authenticated.is_new_registration,
        message,
    }))
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get(COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(name, value)| (name == "mctai_session").then(|| value.to_owned()))
}

fn public_origin(headers: &HeaderMap, config: &AppConfig) -> String {
    if let Some(origin) = config.public_origin.as_deref() {
        return origin.trim_end_matches('/').to_owned();
    }

    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("https");
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(HOST))
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost:8080");

    format!("{proto}://{host}")
}

fn string_claim(claims: &Value, key: &'static str) -> Option<String> {
    claims
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
