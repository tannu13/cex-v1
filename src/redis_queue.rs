use redis::{Client, aio::ConnectionManager};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::config::AppConfig;

#[derive(Clone)]
pub struct RedisQueueClient {
    connection: ConnectionManager,
    request_queue: String,
    brpop_timeout_seconds: usize,
}

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl RedisQueueClient {
    pub async fn from_config(config: AppConfig) -> Result<Self, QueueError> {
        let client = Client::open(config.redis_url)?;
        let connection = client.get_connection_manager().await?;

        Ok(Self {
            connection,
            request_queue: config.redis_request_queue,
            brpop_timeout_seconds: config.redis_brpop_timeout_seconds,
        })
    }

    pub async fn from_env() -> Result<Self, QueueStartupError> {
        let config = AppConfig::from_env()?;
        Ok(Self::from_config(config).await?)
    }

    pub async fn push_response_to<T>(
        &self,
        queue_name: impl AsRef<str>,
        response: &T,
    ) -> Result<(), QueueError>
    where
        T: Serialize,
    {
        let payload = serde_json::to_string(response)?;
        self.push_raw_response_to(queue_name, payload).await
    }

    pub async fn push_raw_response_to(
        &self,
        queue_name: impl AsRef<str>,
        payload: impl AsRef<str>,
    ) -> Result<(), QueueError> {
        let mut connection = self.connection.clone();

        redis::cmd("LPUSH")
            .arg(queue_name.as_ref())
            .arg(payload.as_ref())
            .query_async::<()>(&mut connection)
            .await?;

        Ok(())
    }

    pub async fn pop_request<T>(&self) -> Result<Option<T>, QueueError>
    where
        T: DeserializeOwned,
    {
        let Some(payload) = self.pop_raw_request().await? else {
            return Ok(None);
        };

        Ok(Some(serde_json::from_str(&payload)?))
    }

    pub async fn pop_raw_request(&self) -> Result<Option<String>, QueueError> {
        let mut connection = self.connection.clone();

        let item = redis::cmd("BRPOP")
            .arg(&self.request_queue)
            .arg(self.brpop_timeout_seconds)
            .query_async::<Option<(String, String)>>(&mut connection)
            .await?;

        Ok(item.map(|(_, payload)| payload))
    }
}

#[derive(Debug, Error)]
pub enum QueueStartupError {
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),

    #[error(transparent)]
    Queue(#[from] QueueError),
}
