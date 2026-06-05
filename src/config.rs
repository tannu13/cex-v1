use std::env;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub redis_url: String,
    pub redis_request_queue: String,
    pub redis_brpop_timeout_seconds: usize,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable {name}")]
    MissingVar { name: &'static str },

    #[error("environment variable {name} must not be empty")]
    EmptyVar { name: &'static str },

    #[error("environment variable {name} must be a positive integer")]
    InvalidPositiveInteger { name: &'static str },
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        Ok(Self {
            redis_url: required_string("REDIS_URL")?,
            redis_request_queue: required_string("REDIS_REQUEST_QUEUE")?,
            redis_brpop_timeout_seconds: optional_positive_usize("REDIS_BRPOP_TIMEOUT_SECONDS", 5)?,
        })
    }
}

fn required_string(name: &'static str) -> Result<String, ConfigError> {
    let value = env::var(name).map_err(|_| ConfigError::MissingVar { name })?;
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(ConfigError::EmptyVar { name });
    }

    Ok(trimmed.to_owned())
}

fn optional_positive_usize(name: &'static str, fallback: usize) -> Result<usize, ConfigError> {
    let Ok(value) = env::var(name) else {
        return Ok(fallback);
    };

    let parsed = value
        .trim()
        .parse::<usize>()
        .map_err(|_| ConfigError::InvalidPositiveInteger { name })?;

    if parsed == 0 {
        return Err(ConfigError::InvalidPositiveInteger { name });
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_positive_usize_returns_default_when_missing() {
        let name = "CEX_TEST_MISSING_NUMBER";
        unsafe {
            env::remove_var(name);
        }

        assert_eq!(optional_positive_usize(name, 7).unwrap(), 7);
    }

    #[test]
    fn optional_positive_usize_rejects_zero() {
        let name = "CEX_TEST_ZERO_NUMBER";
        unsafe {
            env::set_var(name, "0");
        }

        assert!(matches!(
            optional_positive_usize(name, 7),
            Err(ConfigError::InvalidPositiveInteger { .. })
        ));

        unsafe {
            env::remove_var(name);
        }
    }
}
