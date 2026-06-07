use std::collections::{BTreeMap, HashMap};

use cex_v1::{
    redis_queue::RedisQueueClient,
    requests::{CreateOrderPayload, QueueRequest, QueueResponse},
};

use serde_json::json;

mod models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queue = RedisQueueClient::from_env().await?;

    loop {
        let Some(payload) = queue.pop_raw_request().await? else {
            continue;
        };

        match serde_json::from_str::<QueueRequest>(&payload) {
            Ok(request) => {
                match &request {
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
