use std::collections::HashMap;

use cex_v1::requests::{CreateOrderPayload, InitBalancePayload, QueueRequest, QueueResponse};
use rust_decimal::dec;
use serde_json::{Value, json};

use crate::models::store::{Balance, Store};

pub struct Engine {
    pub store: Store,
}

impl Engine {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub fn handle(
        &mut self,
        request: QueueRequest,
    ) -> Result<QueueResponse<Value>, QueueResponse<Value>> {
        match &request {
            QueueRequest::InitUserBalance { payload, .. } => {
                let InitBalancePayload { user_id } = payload;
                self.store
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

                println!("{:?}", self.store);
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

                let orderbook = match self.store.orderbooks.get_mut(symbol) {
                    Some(ob) => ob,
                    None => {
                        let response = QueueResponse {
                            correlation_id: request.correlation_id().to_owned(),
                            ok: false,
                            data: None,
                            error: Some(format!("Orderbook does not exist for symbol {}", symbol)),
                        };
                        return Err(response);
                    }
                };
            }
            _ => {
                println!("not a create_order request");
            }
        }

        let response = QueueResponse {
            correlation_id: request.correlation_id().to_owned(),
            ok: true,
            data: Some(json!({
                "orderId": "currentOrderId",
                "status": "open",
                "filledQty": 0,
                "averagePrice": "",
                "fills": [],
            })),
            error: None,
        };
        Ok(response)
    }
}
