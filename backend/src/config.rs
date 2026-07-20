use std::{env, net::SocketAddr};

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Error)]
pub enum ConfigError {
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
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_owned());
        let port = match env::var("PORT") {
            Ok(value) => value
                .parse::<u16>()
                .map_err(|source| ConfigError::InvalidPort { value, source })?,
            Err(_) => 8080,
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
