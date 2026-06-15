use std::{
    collections::HashMap,
    ops::Bound::{Excluded, Unbounded},
};

use crate::{
    models::store::{
        Balance, Fill, OrderBook, OrderRecord, OrderSide, OrderStatus, OrderType, Store,
    },
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

const PRIMARY_CURRENCY: &str = "INR";
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
                            PRIMARY_CURRENCY.to_string(),
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

                if !self.store.balances.contains_key(user_id) {
                    let response = QueueResponse {
                        correlation_id: request.correlation_id().to_owned(),
                        ok: false,
                        data: None,
                        error: Some(format!("User {} have no balance on the server", user_id)),
                    };
                    return Err(response);
                }

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
                            .expect("validated earlier");
                        let mut best_next_price = get_next_best_ask_price(orderbook, None);
                        // create a scope and consume it within it
                        let total_price = price * qty;

                        let available_balance = self
                            .store
                            .balances
                            .get(user_id)
                            .and_then(|b| b.get(PRIMARY_CURRENCY))
                            .map_or(dec!(0), |b| b.available);

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

                            let mut remove_price_level = false;
                            let orders_at_price = orderbook
                                .asks
                                .get_mut(&best_price)
                                .expect("order at best price was validated earlier");
                            while remaining_qty > dec!(0) {
                                let mut remove_front_order = false;
                                if let Some(resting_order) = orders_at_price.front_mut() {
                                    let available_qty =
                                        resting_order.qty - resting_order.filled_qty;

                                    let fill_id = Uuid::new_v4().to_string();
                                    let mut fill = Fill {
                                        fill_id,
                                        symbol: symbol.clone(),
                                        price: best_price,
                                        qty: remaining_qty,
                                        buy_order_id: current_order_id.clone(),
                                        sell_order_id: resting_order.order_id.clone(),
                                        created_at: Utc::now(),
                                    };
                                    if available_qty > remaining_qty {
                                        let current_order = self
                                            .store
                                            .orders
                                            .entry(current_order_id.clone())
                                            .or_insert_with(|| OrderRecord {
                                                order_id: current_order_id.clone(),
                                                user_id: user_id.clone(),
                                                side: OrderSide::Buy,
                                                order_type: OrderType::Limit,
                                                symbol: symbol.clone(),
                                                price: best_next_price,
                                                qty: remaining_qty,
                                                filled_qty: dec!(0),
                                                status: OrderStatus::Filled,
                                                fills: vec![],
                                                created_at: Utc::now(),
                                            });
                                        current_order.filled_qty += remaining_qty;
                                        current_order.fills.push(fill.clone());

                                        resting_order.filled_qty += remaining_qty;
                                        resting_order.status = OrderStatus::PartialFilled;
                                        let seller_user_id = resting_order.user_id.clone();

                                        let resting_order_record = self
                                            .store
                                            .orders
                                            .entry(resting_order.order_id.clone())
                                            .or_insert_with(|| OrderRecord {
                                                order_id: resting_order.order_id.clone(),
                                                user_id: resting_order.user_id.clone(),
                                                side: resting_order.side.clone(),
                                                order_type: resting_order.order_type.clone(),
                                                symbol: symbol.clone(),
                                                price: best_next_price,
                                                qty: remaining_qty,
                                                filled_qty: resting_order.filled_qty,
                                                status: OrderStatus::PartialFilled,
                                                fills: vec![],
                                                created_at: Utc::now(),
                                            });
                                        resting_order_record.status = OrderStatus::PartialFilled;
                                        resting_order_record.filled_qty = resting_order.filled_qty;
                                        resting_order_record.fills.push(fill.clone());

                                        let fill_qty = fill.qty;
                                        self.store.fills.push(fill);

                                        let price_for_filled_qty = fill_qty * best_price;
                                        {
                                            let user_balance = self
                                                .store
                                                .balances
                                                .get_mut(user_id)
                                                .expect("validated earlier");
                                            let currency_balance = user_balance
                                                .get_mut(PRIMARY_CURRENCY)
                                                .expect("validated earlier");
                                            currency_balance.available -= price_for_filled_qty;

                                            let symbol_balance = user_balance
                                                .entry(symbol.clone())
                                                .or_insert(Balance {
                                                    available: dec!(0),
                                                    locked: dec!(0),
                                                });
                                            symbol_balance.available += fill_qty;
                                        }

                                        {
                                            let seller_balance = self
                                                .store
                                                .balances
                                                .entry(seller_user_id)
                                                .or_insert(HashMap::from([
                                                    (
                                                        PRIMARY_CURRENCY.to_string(),
                                                        Balance {
                                                            available: dec!(0),
                                                            locked: dec!(0),
                                                        },
                                                    ),
                                                    (
                                                        symbol.clone(),
                                                        Balance {
                                                            available: dec!(0),
                                                            locked: dec!(0),
                                                        },
                                                    ),
                                                ]));

                                            let currency_balance = seller_balance
                                                .get_mut(PRIMARY_CURRENCY)
                                                .expect("validated earlier");
                                            currency_balance.available += price_for_filled_qty;

                                            let symbol_balance = seller_balance
                                                .get_mut(symbol)
                                                .expect("validated earlier");
                                            symbol_balance.locked -= fill_qty;
                                        }

                                        remaining_qty = dec!(0);
                                        break;
                                    } else if available_qty == remaining_qty {
                                        let current_order = self
                                            .store
                                            .orders
                                            .entry(current_order_id.clone())
                                            .or_insert_with(|| OrderRecord {
                                                order_id: current_order_id.clone(),
                                                user_id: user_id.clone(),
                                                side: OrderSide::Buy,
                                                order_type: OrderType::Limit,
                                                symbol: symbol.clone(),
                                                price: best_next_price,
                                                qty: remaining_qty,
                                                filled_qty: dec!(0),
                                                status: OrderStatus::Filled,
                                                fills: vec![],
                                                created_at: Utc::now(),
                                            });
                                        current_order.filled_qty += remaining_qty;
                                        current_order.fills.push(fill.clone());

                                        resting_order.filled_qty += remaining_qty;
                                        resting_order.status = OrderStatus::Filled;

                                        let resting_order_record = self
                                            .store
                                            .orders
                                            .entry(resting_order.order_id.clone())
                                            .or_insert_with(|| OrderRecord {
                                                order_id: resting_order.order_id.clone(),
                                                user_id: resting_order.user_id.clone(),
                                                side: OrderSide::Buy,
                                                order_type: OrderType::Limit,
                                                symbol: symbol.clone(),
                                                price: best_next_price,
                                                qty: resting_order.qty,
                                                filled_qty: dec!(0),
                                                status: OrderStatus::Filled,
                                                fills: vec![],
                                                created_at: Utc::now(),
                                            });
                                        resting_order_record.status = OrderStatus::Filled;
                                        resting_order_record.filled_qty = resting_order.filled_qty;
                                        resting_order_record.fills.push(fill.clone());
                                        let seller_user_id = resting_order_record.user_id.clone();

                                        let fill_qty = fill.qty;
                                        self.store.fills.push(fill);

                                        let price_for_filled_qty = fill_qty * best_price;
                                        {
                                            let user_balance = self
                                                .store
                                                .balances
                                                .get_mut(user_id)
                                                .expect("validated earlier");
                                            let currency_balance = user_balance
                                                .get_mut(PRIMARY_CURRENCY)
                                                .expect("validated earlier");
                                            currency_balance.available -= price_for_filled_qty;

                                            let symbol_balance = user_balance
                                                .entry(symbol.clone())
                                                .or_insert_with(|| Balance {
                                                    available: dec!(0),
                                                    locked: dec!(0),
                                                });
                                            symbol_balance.available += fill_qty;
                                        }

                                        {
                                            let seller_balance = self
                                                .store
                                                .balances
                                                .entry(seller_user_id.clone())
                                                .or_insert_with(|| {
                                                    HashMap::from([
                                                        (
                                                            PRIMARY_CURRENCY.to_string(),
                                                            Balance {
                                                                available: dec!(0),
                                                                locked: dec!(0),
                                                            },
                                                        ),
                                                        (
                                                            symbol.to_string(),
                                                            Balance {
                                                                available: dec!(0),
                                                                locked: dec!(0),
                                                            },
                                                        ),
                                                    ])
                                                });
                                            let currency_balance = seller_balance
                                                .get_mut(PRIMARY_CURRENCY)
                                                .expect("validated earlier");
                                            currency_balance.available += price_for_filled_qty;

                                            let symbol_balance = seller_balance
                                                .get_mut(symbol)
                                                .expect("validated earlier");
                                            symbol_balance.locked -= fill_qty;
                                        }

                                        remaining_qty = dec!(0);
                                        remove_front_order = true;

                                        // td: move filled resting_order out of orderbook when they are filled
                                    } else {
                                        // available_qty < remaining_qty
                                        remaining_qty -= available_qty;
                                        fill.qty = available_qty;

                                        let current_order = self
                                            .store
                                            .orders
                                            .entry(current_order_id.clone())
                                            .or_insert_with(|| OrderRecord {
                                                order_id: current_order_id.clone(),
                                                user_id: user_id.clone(),
                                                side: OrderSide::Buy,
                                                order_type: OrderType::Limit,
                                                symbol: symbol.clone(),
                                                price: best_next_price,
                                                qty,
                                                filled_qty: dec!(0),
                                                status: OrderStatus::PartialFilled,
                                                fills: vec![],
                                                created_at: Utc::now(),
                                            });
                                        current_order.filled_qty += available_qty;
                                        current_order.fills.push(fill.clone());

                                        resting_order.filled_qty += available_qty;
                                        resting_order.status = OrderStatus::Filled;
                                        let seller_user_id = resting_order.user_id.clone();

                                        let resting_order_record = self
                                            .store
                                            .orders
                                            .entry(resting_order.order_id.clone())
                                            .or_insert_with(|| OrderRecord {
                                                order_id: resting_order.order_id.clone(),
                                                user_id: resting_order.user_id.clone(),
                                                side: OrderSide::Sell,
                                                order_type: OrderType::Limit,
                                                symbol: symbol.clone(),
                                                price: best_next_price,
                                                qty: resting_order.qty,
                                                filled_qty: dec!(0),
                                                status: OrderStatus::Filled,
                                                fills: vec![],
                                                created_at: resting_order.created_at,
                                            });
                                        resting_order_record.filled_qty = resting_order.filled_qty;
                                        resting_order_record.status = OrderStatus::Filled;
                                        resting_order_record.fills.push(fill.clone());

                                        let fill_qty = fill.qty;
                                        self.store.fills.push(fill);

                                        let price_for_filled_qty = fill_qty * best_price;
                                        {
                                            let user_balance = self
                                                .store
                                                .balances
                                                .get_mut(user_id)
                                                .expect("user balances validated earlier");
                                            let currency_balance = user_balance.get_mut(PRIMARY_CURRENCY).expect("user's primary currency balance validated earlier");
                                            currency_balance.available -= price_for_filled_qty;

                                            let symbol_balance = user_balance
                                                .entry(symbol.clone())
                                                .or_insert_with(|| Balance {
                                                    available: dec!(0),
                                                    locked: dec!(0),
                                                });
                                            symbol_balance.available += fill_qty;
                                        }

                                        {
                                            let seller_balance = self
                                                .store
                                                .balances
                                                .entry(seller_user_id)
                                                .or_insert_with(|| {
                                                    HashMap::from([
                                                        (
                                                            PRIMARY_CURRENCY.to_owned(),
                                                            Balance {
                                                                available: dec!(0),
                                                                locked: dec!(0),
                                                            },
                                                        ),
                                                        (
                                                            symbol.to_owned(),
                                                            Balance {
                                                                available: dec!(0),
                                                                locked: dec!(0),
                                                            },
                                                        ),
                                                    ])
                                                });

                                            let currency_balance = seller_balance
                                                .get_mut(PRIMARY_CURRENCY)
                                                .expect("added currency balance default above");
                                            currency_balance.available += price_for_filled_qty;

                                            let symbol_balance = seller_balance
                                                .get_mut(symbol)
                                                .expect("add symbol balance default above");
                                            symbol_balance.locked -= fill_qty;
                                        }

                                        remaining_qty = dec!(0);
                                        remove_front_order = true;
                                    }
                                }

                                if remove_front_order {
                                    orders_at_price.pop_front();

                                    if orders_at_price.is_empty() {
                                        remove_price_level = true;
                                    }

                                    break;
                                }
                            }

                            if remove_price_level {
                                orderbook.asks.remove(&best_price);
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
