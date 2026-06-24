use std::collections::{HashMap, VecDeque, btree_map::Entry};

use crate::{
    models::store::{
        Balance, OrderRecord, OrderSide, OrderStatus, OrderType, PRIMARY_CURRENCY, RestingOrder,
        Store,
    },
    requests::{CreateOrderPayload, InitBalancePayload, QueueRequest, QueueResponse},
};
use chrono::Utc;
use rust_decimal::{Decimal, dec, prelude::FromPrimitive};
use serde_json::{Value, json};
use uuid::Uuid;

pub struct Engine {
    pub store: Store,
}

impl Engine {
    fn validate_limit_order(
        &self,
        user_id: &str,
        check_balance_for: &str,
        symbol: &str,
        total_price: Decimal,
    ) -> Result<(), &str> {
        let available_balance = self
            .store
            .balances
            .accounts
            .get(user_id)
            .and_then(|b| b.get(check_balance_for))
            .map_or(dec!(0), |b| b.available);

        if available_balance < total_price {
            return Err("User has insufficient balance");
        }

        self.store
            .orderbooks
            .get(symbol)
            .ok_or_else(|| "orderbook not found")?;
        return Ok(());
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
                    .accounts
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

                if !self.store.balances.accounts.contains_key(user_id) {
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

                        let total_price = price * qty;
                        self.validate_limit_order(user_id, PRIMARY_CURRENCY, symbol, total_price)
                            .map_err(|msg| QueueResponse {
                                correlation_id: request.correlation_id().to_owned(),
                                ok: false,
                                data: None,
                                error: Some(msg.to_string()),
                            })?;

                        let orderbook = self
                            .store
                            .orderbooks
                            .get_mut(symbol)
                            .expect("validated earlier");

                        let match_result = orderbook.match_limit(user_id, side.clone(), price, qty);
                        match self.store.balances.apply_fills(&match_result.fills, symbol) {
                            Ok(()) => (),
                            Err(_) => {
                                // todo:: handle this better via In-Memory Aggregation or the Scratchpad Pattern in apply fills
                                /*
                                first loop through fills collects all the deltas in a hash map of user_id + symbol -> BalanceDelta
                                pub struct BalanceDelta {
                                    pub available_change: Decimal,
                                    pub locked_change: Decimal,
                                    }
                                    then a loop through balances to validate all users have apt balances for symbols as per the delta info
                                    then final loop to commit to each user's balance
                                    */
                                panic!("Fills failed to be applied to balances");
                            }
                        }

                        self.store.record_match(
                            user_id,
                            current_order_id.clone(),
                            symbol,
                            side.clone(),
                            order_type.clone(),
                            qty,
                            &match_result,
                        );

                        if match_result.remaining_qty > dec!(0) {
                            let fill_qty = match_result.filled_qty;
                            let current_order = RestingOrder {
                                order_id: current_order_id.clone(),
                                user_id: user_id.to_owned(),
                                side: side.to_owned(),
                                order_type: order_type.to_owned(),
                                symbol: symbol.to_owned(),
                                price: price.to_owned(),
                                qty: qty.to_owned(),
                                filled_qty: fill_qty,
                                status: match_result.taker_final_status,
                                created_at: Utc::now(),
                            };

                            let orderbook = self
                                .store
                                .orderbooks
                                .get_mut(symbol)
                                .expect("validated earlier");
                            match orderbook.bids.entry(price) {
                                Entry::Vacant(entry) => {
                                    entry.insert(VecDeque::from([current_order]));
                                }
                                Entry::Occupied(mut entry) => {
                                    entry.get_mut().push_back(current_order);
                                }
                            };

                            if fill_qty == dec!(0) {
                                self.store.orders.insert(
                                    current_order_id.to_owned(),
                                    OrderRecord {
                                        order_id: current_order_id.clone(),
                                        user_id: user_id.to_owned(),
                                        side: side.to_owned(),
                                        order_type: order_type.to_owned(),
                                        symbol: symbol.to_owned(),
                                        price: Some(price.to_owned()),
                                        qty,
                                        filled_qty: dec!(0),
                                        status: OrderStatus::Open,
                                        fills: vec![],
                                        created_at: Utc::now(),
                                    },
                                );
                            }

                            let remaining_total_price = price * match_result.remaining_qty;
                            let user_balance = self
                                .store
                                .balances
                                .accounts
                                .get_mut(user_id)
                                .expect("user balances validated earlier");
                            let currency_balance = user_balance
                                .get_mut(PRIMARY_CURRENCY)
                                .expect("user's primary currency balance validated earlier");
                            currency_balance.available -= remaining_total_price;
                            currency_balance.locked += remaining_total_price;
                        }
                    } else if side == &OrderSide::Sell {
                        let price = Decimal::from_f64(*price).unwrap_or(dec!(0));
                        let qty = Decimal::from_f64(*qty).unwrap_or(dec!(0));

                        let total_price = price * qty;
                        self.validate_limit_order(user_id, symbol, symbol, total_price)
                            .map_err(|msg| QueueResponse {
                                correlation_id: request.correlation_id().to_owned(),
                                ok: false,
                                data: None,
                                error: Some(msg.to_string()),
                            })?;

                        let orderbook = self
                            .store
                            .orderbooks
                            .get_mut(symbol)
                            .expect("validated earlier");

                        let match_result = orderbook.match_limit(user_id, side.clone(), price, qty);
                        match self.store.balances.apply_fills(&match_result.fills, symbol) {
                            Ok(()) => (),
                            Err(_) => {
                                // todo:: handle this better via In-Memory Aggregation or the Scratchpad Pattern in apply fills
                                /*
                                first loop through fills collects all the deltas in a hash map of user_id + symbol -> BalanceDelta
                                pub struct BalanceDelta {
                                    pub available_change: Decimal,
                                    pub locked_change: Decimal,
                                    }
                                    then a loop through balances to validate all users have apt balances for symbols as per the delta info
                                    then final loop to commit to each user's balance
                                    */
                                panic!("Fills failed to be applied to balances");
                            }
                        }

                        self.store.record_match(
                            user_id,
                            current_order_id.clone(),
                            symbol,
                            side.clone(),
                            order_type.clone(),
                            qty,
                            &match_result,
                        );

                        if match_result.remaining_qty > dec!(0) {
                            let fill_qty = match_result.filled_qty;
                            let current_order = RestingOrder {
                                order_id: current_order_id.clone(),
                                user_id: user_id.to_owned(),
                                side: side.to_owned(),
                                order_type: order_type.to_owned(),
                                symbol: symbol.to_owned(),
                                price: price.to_owned(),
                                qty: qty.to_owned(),
                                filled_qty: fill_qty,
                                status: match_result.taker_final_status,
                                created_at: Utc::now(),
                            };

                            let orderbook = self
                                .store
                                .orderbooks
                                .get_mut(symbol)
                                .expect("validated earlier");
                            match orderbook.bids.entry(price) {
                                Entry::Vacant(entry) => {
                                    entry.insert(VecDeque::from([current_order]));
                                }
                                Entry::Occupied(mut entry) => {
                                    entry.get_mut().push_back(current_order);
                                }
                            };

                            if fill_qty == dec!(0) {
                                self.store.orders.insert(
                                    current_order_id.to_owned(),
                                    OrderRecord {
                                        order_id: current_order_id.clone(),
                                        user_id: user_id.to_owned(),
                                        side: side.to_owned(),
                                        order_type: order_type.to_owned(),
                                        symbol: symbol.to_owned(),
                                        price: Some(price.to_owned()),
                                        qty,
                                        filled_qty: dec!(0),
                                        status: OrderStatus::Open,
                                        fills: vec![],
                                        created_at: Utc::now(),
                                    },
                                );
                            }

                            let remaining_total_price = price * match_result.remaining_qty;
                            let user_balance = self
                                .store
                                .balances
                                .accounts
                                .get_mut(user_id)
                                .expect("user balances validated earlier");
                            let currency_balance = user_balance
                                .get_mut(PRIMARY_CURRENCY)
                                .expect("user's primary currency balance validated earlier");
                            currency_balance.available -= remaining_total_price;
                            currency_balance.locked += remaining_total_price;
                        }
                    }
                }
                println!("{:#?}", self.store);
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
