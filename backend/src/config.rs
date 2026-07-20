use std::{env, net::SocketAddr};

use thiserror::Error;

#[derive(Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub public_origin: Option<String>,
    pub allowed_cors_origin: Option<String>,
    pub auth: Option<AuthConfig>,
    pub email: Option<EmailConfig>,
    pub object_storage: Option<ObjectStorageConfig>,
    pub instagram: Option<InstagramConfig>,
    pub generation: Option<GenerationConfig>,
}

#[derive(Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Clone)]
pub struct AuthConfig {
    pub url: String,
    pub app_token: String,
    pub jwks_url: String,
}

#[derive(Clone)]
pub struct EmailConfig {
    pub url: String,
    pub app_token: String,
}

#[derive(Clone)]
pub struct ObjectStorageConfig {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub prefix: String,
}

#[derive(Clone)]
pub struct InstagramConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Clone)]
pub struct GenerationConfig {
    pub openai_api_key: String,
    pub image_model: String,
    pub caption_model: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{key} must be set")]
    MissingRequired { key: &'static str },
    #[error("{group} configuration is incomplete; missing {missing:?}")]
    IncompleteGroup {
        group: &'static str,
        missing: Vec<&'static str>,
    },
    #[error("PORT must be a valid u16, got {value:?}")]
    InvalidPort {
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("server address must be valid, got {value:?}")]
    InvalidAddress {
        value: String,
        #[source]
        source: std::net::AddrParseError,
    },
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            server: ServerConfig::from_env()?,
            database: DatabaseConfig {
                url: required_env("DATABASE_URL")?,
            },
            public_origin: optional_env("SELF_URL"),
            allowed_cors_origin: optional_env("ALLOWED_CORS_ORIGIN"),
            auth: AuthConfig::from_env()?,
            email: EmailConfig::from_env()?,
            object_storage: ObjectStorageConfig::from_env()?,
            instagram: InstagramConfig::from_env()?,
            generation: GenerationConfig::from_env()?,
        })
    }
}

impl ServerConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let host = optional_env("HOST").unwrap_or_else(|| "0.0.0.0".to_owned());
        let port = match optional_env("PORT") {
            Some(value) => value
                .parse::<u16>()
                .map_err(|source| ConfigError::InvalidPort { value, source })?,
            None => 8080,
        };

        Ok(Self { host, port })
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, ConfigError> {
        let value = format!("{}:{}", self.host, self.port);
        value
            .parse::<SocketAddr>()
            .map_err(|source| ConfigError::InvalidAddress { value, source })
    }
}

impl AuthConfig {
    fn from_env() -> Result<Option<Self>, ConfigError> {
        let Some(values) = optional_group(
            "MCTAI auth",
            &["MCTAI_AUTH_URL", "MCTAI_AUTH_APP_TOKEN", "MCTAI_AUTH_JWKS_URL"],
        )?
        else {
            return Ok(None);
        };

        Ok(Some(Self {
            url: values[0].clone(),
            app_token: values[1].clone(),
            jwks_url: values[2].clone(),
        }))
    }
}

impl EmailConfig {
    fn from_env() -> Result<Option<Self>, ConfigError> {
        let Some(values) =
            optional_group("MCTAI email", &["MCTAI_EMAIL_URL", "MCTAI_EMAIL_APP_TOKEN"])?
        else {
            return Ok(None);
        };

        Ok(Some(Self {
            url: values[0].clone(),
            app_token: values[1].clone(),
        }))
    }
}

impl ObjectStorageConfig {
    fn from_env() -> Result<Option<Self>, ConfigError> {
        let Some(values) = optional_group(
            "object storage",
            &[
                "OBJECT_STORAGE_ENDPOINT",
                "OBJECT_STORAGE_REGION",
                "OBJECT_STORAGE_BUCKET",
                "OBJECT_STORAGE_ACCESS_KEY_ID",
                "OBJECT_STORAGE_SECRET_ACCESS_KEY",
                "OBJECT_STORAGE_PREFIX",
            ],
        )?
        else {
            return Ok(None);
        };

        Ok(Some(Self {
            endpoint: values[0].clone(),
            region: values[1].clone(),
            bucket: values[2].clone(),
            access_key_id: values[3].clone(),
            secret_access_key: values[4].clone(),
            prefix: values[5].clone(),
        }))
    }
}

impl InstagramConfig {
    fn from_env() -> Result<Option<Self>, ConfigError> {
        let Some(values) = optional_group(
            "Instagram app",
            &[
                "INSTAGRAM_CLIENT_ID",
                "INSTAGRAM_CLIENT_SECRET",
                "INSTAGRAM_REDIRECT_URI",
            ],
        )?
        else {
            return Ok(None);
        };

        Ok(Some(Self {
            client_id: values[0].clone(),
            client_secret: values[1].clone(),
            redirect_uri: values[2].clone(),
        }))
    }
}

impl GenerationConfig {
    fn from_env() -> Result<Option<Self>, ConfigError> {
        let api_key = optional_env("OPENAI_API_KEY");
        let image_model = optional_env("IMAGE_GENERATION_MODEL");
        let caption_model = optional_env("CAPTION_GENERATION_MODEL");

        if api_key.is_none() && (image_model.is_some() || caption_model.is_some()) {
            return Err(ConfigError::IncompleteGroup {
                group: "generation",
                missing: vec!["OPENAI_API_KEY"],
            });
        }

        let Some(openai_api_key) = api_key else {
            return Ok(None);
        };

        Ok(Some(Self {
            openai_api_key,
            image_model: image_model.unwrap_or_else(|| "gpt-image-1".to_owned()),
            caption_model: caption_model.unwrap_or_else(|| "gpt-4.1-mini".to_owned()),
        }))
    }
}

fn required_env(key: &'static str) -> Result<String, ConfigError> {
    optional_env(key).ok_or(ConfigError::MissingRequired { key })
}

fn optional_env(key: &'static str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

fn optional_group(
    group: &'static str,
    keys: &[&'static str],
) -> Result<Option<Vec<String>>, ConfigError> {
    let values = keys
        .iter()
        .map(|key| optional_env(key))
        .collect::<Vec<Option<String>>>();

    if values.iter().all(Option::is_none) {
        return Ok(None);
    }

    let missing = keys
        .iter()
        .zip(values.iter())
        .filter_map(|(key, value)| value.is_none().then_some(*key))
        .collect::<Vec<&'static str>>();

    if !missing.is_empty() {
        return Err(ConfigError::IncompleteGroup { group, missing });
    }

    Ok(Some(values.into_iter().flatten().collect()))
}
