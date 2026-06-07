use std::collections::HashMap;

use cex_v1::{
    redis_queue::RedisQueueClient,
    requests::{CreateOrderPayload, InitBalancePayload, QueueRequest, QueueResponse},
};

use rust_decimal::dec;
use serde_json::json;

use crate::models::store::{Balance, create_exchange_store};

mod models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = create_exchange_store();
    let queue = RedisQueueClient::from_env().await?;

    loop {
        let Some(payload) = queue.pop_raw_request().await? else {
            continue;
        };

        match serde_json::from_str::<QueueRequest>(&payload) {
            Ok(request) => {
                match &request {
                    QueueRequest::InitUserBalance { payload, .. } => {
                        let InitBalancePayload { user_id } = payload;
                        store
                            .balances
                            .entry(user_id.clone())
                            .or_insert(HashMap::from([
                                (
                                    "INR".to_string(),
                                    Balance {
                                        available: dec!(1000),
                                        locked: dec!(0),
                                    },
                                ),
                                (
                                    "SOL".to_string(),
                                    Balance {
                                        available: dec!(1000),
                                        locked: dec!(0),
                                    },
                                ),
                                (
                                    "BTC".to_string(),
                                    Balance {
                                        available: dec!(1000),
                                        locked: dec!(0),
                                    },
                                ),
                            ]));

                        println!("{:?}", store);
                    }
                    QueueRequest::CreateOrder { payload, .. } => {
                        let CreateOrderPayload {
                            user_id,
                            order_type,
                            side,
                            symbol,
                            price,
                            qty,
                        } = payload;
                    }
                    _ => {
                        println!("not a create_order request");
                    }
                }

                let response = QueueResponse {
                    correlation_id: request.correlation_id().to_owned(),
                    ok: true,
                    data: json!({
                        "orderId": "currentOrderId",
                        "status": "open",
                        "filledQty": 0,
                        "averagePrice": "",
                        "fills": [],
                    }),
                };

                queue
                    .push_response_to(request.response_queue(), &response)
                    .await?;
            }
            Err(error) => {
                eprintln!("request payload does not match a known request type: {error}");
            }
        }
    }
}
