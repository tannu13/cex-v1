use std::{
    collections::HashMap,
    ops::Bound::{Excluded, Unbounded},
};

use crate::{
    models::store::{Balance, Fill, OrderBook, OrderSide, OrderType, Store},
    requests::{CreateOrderPayload, InitBalancePayload, QueueRequest, QueueResponse},
};
use chrono::Utc;
use rust_decimal::{Decimal, dec, prelude::FromPrimitive};
use serde_json::{Value, json};
use uuid::Uuid;

fn get_next_best_ask_price(orderbook: &OrderBook, start_from: Option<Decimal>) -> Option<Decimal> {
    match start_from {
        None => orderbook.asks.first_key_value().map(|(price, _)| *price),
        Some(start) => orderbook
            .asks
            .range((Excluded(&start), Unbounded))
            .next()
            .map(|(price, _)| *price),
    }
}

fn get_next_best_bid_price(orderbook: &OrderBook, start_from: Option<Decimal>) -> Option<Decimal> {
    match start_from {
        None => orderbook.bids.last_key_value().map(|(price, _)| *price),
        Some(start) => orderbook
            .bids
            .range((Unbounded, Excluded(&start)))
            .next_back()
            .map(|(price, _)| *price),
    }
}

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

                if !self.store.orderbooks.contains_key(symbol) {
                    let response = QueueResponse {
                        correlation_id: request.correlation_id().to_owned(),
                        ok: false,
                        data: None,
                        error: Some(format!("Orderbook does not exist for symbol {}", symbol)),
                    };
                    return Err(response);
                }
                // let orderbook = match self.store.orderbooks.get_mut(symbol) {
                //     Some(ob) => ob,
                //     None => {
                //         let response = QueueResponse {
                //             correlation_id: request.correlation_id().to_owned(),
                //             ok: false,
                //             data: None,
                //             error: Some(format!("Orderbook does not exist for symbol {}", symbol)),
                //         };
                //         return Err(response);
                //     }
                // };

                let user_balance = match self.store.balances.get_mut(user_id) {
                    Some(b) => b,
                    None => {
                        let response = QueueResponse {
                            correlation_id: request.correlation_id().to_owned(),
                            ok: false,
                            data: None,
                            error: Some(format!("User {} have no balance on the server", user_id)),
                        };
                        return Err(response);
                    }
                };

                let current_order_id = Uuid::new_v4().to_string();

                if order_type == &OrderType::Limit
                    && let Some(price) = price
                {
                    if side == &OrderSide::Buy {
                        let price = Decimal::from_f64(*price).unwrap_or(dec!(0));
                        let qty = Decimal::from_f64(*qty).unwrap_or(dec!(0));

                        let orderbook = self
                            .store
                            .orderbooks
                            .get_mut(symbol)
                            .expect("validated above");
                        let mut best_next_price = get_next_best_ask_price(orderbook, None);
                        // create a scope and consume it within it
                        let total_price = price * qty;

                        let available_balance =
                            user_balance.get("INR").map_or(dec!(0), |b| b.available);

                        if available_balance < total_price {
                            let response = QueueResponse {
                                correlation_id: request.correlation_id().to_owned(),
                                ok: false,
                                data: None,
                                error: Some(format!("User has insufficient balance")),
                            };
                            return Err(response);
                        }

                        let mut remaining_qty = qty;
                        while let Some(best_price) = best_next_price {
                            if remaining_qty <= dec!(0) || best_price > price {
                                break;
                            }

                            let orders_at_price = orderbook.asks.get_mut(&best_price).unwrap();
                            while remaining_qty > dec!(0) {
                                if let Some(resting_order) = orders_at_price.front() {
                                    let available_qty =
                                        resting_order.qty - resting_order.filled_qty;

                                    let fill_id = Uuid::new_v4().to_string();
                                    let fill = Fill {
                                        fill_id,
                                        symbol: symbol.clone(),
                                        price: best_price,
                                        qty,
                                        buy_order_id: current_order_id.clone(),
                                        sell_order_id: resting_order.order_id.clone(),
                                        created_at: Utc::now(),
                                    };
                                    if available_qty > remaining_qty {
                                        // current_order_id
                                    }
                                }
                            }

                            // end to update to the best next price
                            best_next_price = get_next_best_ask_price(orderbook, Some(best_price));
                        }
                    }
                }
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
