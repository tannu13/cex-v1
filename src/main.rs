use std::collections::HashMap;

use cex_v1::{
    redis_queue::RedisQueueClient,
    requests::{CreateOrderPayload, InitBalancePayload, QueueRequest, QueueResponse},
};

use rust_decimal::dec;
use serde_json::json;

use crate::{
    models::store::{Balance, create_exchange_store},
    services::engine::Engine,
};

mod models;
mod services;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = create_exchange_store();
    let mut engine = Engine { store };

    let queue = RedisQueueClient::from_env().await?;

    loop {
        let Some(payload) = queue.pop_raw_request().await? else {
            continue;
        };

        match serde_json::from_str::<QueueRequest>(&payload) {
            Ok(request) => match engine.handle(request) {
                Ok(response) => {
                    queue
                        .push_response_to(request.response_queue(), &response)
                        .await?
                }
                Err(error) => {
                    queue
                        .push_response_to(request.response_queue(), &error)
                        .await?
                }
            },
            Err(error) => {
                eprintln!("request payload does not match a known request type: {error}");
            }
        }
    }
}
