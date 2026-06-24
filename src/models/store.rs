use std::{
    cmp::min,
    collections::{BTreeMap, HashMap, VecDeque},
    ops::Bound::{Excluded, Unbounded},
};

use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type UserId = String;
pub type MarketId = String;
pub type OrderId = String;
pub type Currency = String;
pub const PRIMARY_CURRENCY: &str = "INR";

#[derive(Debug)]
pub enum Error {
    UserNotFound,
    BalanceNotFound,
    InsufficientBalance,
    OrderNotFound,
}
#[derive(Debug)]
pub struct Balance {
    pub available: Decimal,
    pub locked: Decimal,
}

#[derive(Debug)]
pub struct Balances {
    pub accounts: HashMap<UserId, HashMap<Currency, Balance>>,
}

impl Balances {
    fn get_balance_mut(&mut self, user_id: &str, symbol: &str) -> &mut HashMap<String, Balance> {
        self.accounts.entry(user_id.to_owned()).or_insert_with(|| {
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
        })
    }
    pub fn apply_fills(&mut self, fills: &[FillEvent], symbol: &str) -> Result<(), Error> {
        for fill in fills {
            let price_for_filled_qty = fill.qty * fill.price;

            match fill.taker_side {
                OrderSide::Buy => {
                    {
                        // taker block
                        let taker_balance = self.get_balance_mut(&fill.taker_user_id, symbol);
                        let currency_balance = taker_balance
                            .get_mut(PRIMARY_CURRENCY)
                            .ok_or(Error::BalanceNotFound)?;
                        currency_balance.available -= price_for_filled_qty;

                        let symbol_balance = taker_balance
                            .get_mut(symbol)
                            .ok_or(Error::BalanceNotFound)?;
                        symbol_balance.available += fill.qty;
                    }

                    {
                        // maker block
                        let maker_balance = self.get_balance_mut(&fill.maker_user_id, symbol);
                        let currency_balance = maker_balance
                            .get_mut(PRIMARY_CURRENCY)
                            .ok_or(Error::BalanceNotFound)?;
                        currency_balance.available += price_for_filled_qty;

                        let symbol_balance = maker_balance
                            .get_mut(symbol)
                            .ok_or(Error::BalanceNotFound)?;
                        symbol_balance.locked -= fill.qty;
                    }
                }
                OrderSide::Sell => {
                    {
                        // taker block
                        let taker_balance = self.get_balance_mut(&fill.taker_user_id, symbol);
                        let currency_balance = taker_balance
                            .get_mut(PRIMARY_CURRENCY)
                            .ok_or(Error::BalanceNotFound)?;
                        currency_balance.available += price_for_filled_qty;

                        let symbol_balance = taker_balance
                            .get_mut(symbol)
                            .ok_or(Error::BalanceNotFound)?;
                        symbol_balance.available -= fill.qty;
                    }

                    {
                        // maker block
                        let maker_balance = self.get_balance_mut(&fill.maker_user_id, symbol);
                        let currency_balance = maker_balance
                            .get_mut(PRIMARY_CURRENCY)
                            .ok_or(Error::BalanceNotFound)?;
                        currency_balance.locked -= price_for_filled_qty;

                        let symbol_balance = maker_balance
                            .get_mut(symbol)
                            .ok_or(Error::BalanceNotFound)?;
                        symbol_balance.available += fill.qty;
                    }
                }
            }
        }

        return Ok(());
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fill {
    pub fill_id: String,
    pub symbol: String,
    pub price: Decimal,
    pub qty: Decimal,
    pub buy_order_id: String,
    pub buy_user_id: String,
    pub sell_order_id: String,
    pub sell_user_id: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Filled,
    Open,
    PartialFilled,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestingOrder {
    pub order_id: String,
    pub user_id: String,
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub symbol: String,
    pub price: Decimal,
    pub qty: Decimal,
    pub filled_qty: Decimal,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRecord {
    pub order_id: String,
    pub user_id: String,
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub symbol: String,
    pub price: Option<Decimal>,
    pub qty: Decimal,
    pub filled_qty: Decimal,
    pub status: OrderStatus,
    pub fills: Vec<Fill>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct OrderBook {
    pub bids: BTreeMap<Decimal, VecDeque<RestingOrder>>,
    pub asks: BTreeMap<Decimal, VecDeque<RestingOrder>>,
}
pub struct MatchResult {
    pub fills: Vec<FillEvent>,
    pub remaining_qty: Decimal,
    pub filled_qty: Decimal,
    pub requested_price: Decimal,
    pub taker_final_status: OrderStatus,
}

pub struct FillEvent {
    pub fill_id: String,
    pub price: Decimal,
    pub qty: Decimal,
    pub maker_user_id: String,
    pub maker_order_id: String,
    pub taker_user_id: String,
    pub taker_side: OrderSide,
    pub is_maker_fully_filled: bool,
    pub maker_total_qty: Decimal,
}

impl OrderBook {
    fn get_next_best_ask_price(&self, start_from: Option<Decimal>) -> Option<Decimal> {
        match start_from {
            None => self.asks.first_key_value().map(|(price, _)| *price),
            Some(start) => self
                .asks
                .range((Excluded(&start), Unbounded))
                .next()
                .map(|(price, _)| *price),
        }
    }
    fn get_next_best_bid_price(&self, start_from: Option<Decimal>) -> Option<Decimal> {
        match start_from {
            None => self.bids.last_key_value().map(|(price, _)| *price),
            Some(start) => self
                .bids
                .range((Unbounded, Excluded(&start)))
                .next_back()
                .map(|(price, _)| *price),
        }
    }

    pub fn match_limit(
        &mut self,
        user_id: &str,
        side: OrderSide,
        price: Decimal,
        qty: Decimal,
    ) -> MatchResult {
        let mut remaining_qty = qty;
        let mut fills = Vec::new();
        let mut best_next_price = match side {
            OrderSide::Buy => self.get_next_best_ask_price(None),
            OrderSide::Sell => self.get_next_best_bid_price(None),
        };

        while let Some(best_price) = best_next_price {
            let price_crossed = match side {
                OrderSide::Buy => best_price <= price,
                OrderSide::Sell => best_price >= price,
            };
            if remaining_qty <= dec!(0) || !price_crossed {
                break;
            }

            let orders_at_price = match side {
                OrderSide::Buy => self
                    .asks
                    .get_mut(&best_price)
                    .expect("order at best price was validated earlier"),
                OrderSide::Sell => self
                    .bids
                    .get_mut(&best_price)
                    .expect("order at best price was validated earlier"),
            };
            while remaining_qty > dec!(0) && !orders_at_price.is_empty() {
                if let Some(resting_order) = orders_at_price.front_mut() {
                    let available_qty = resting_order.qty - resting_order.filled_qty;

                    let fill_id = Uuid::new_v4().to_string();
                    let fill_qty = min(available_qty, remaining_qty);
                    let mut fill_event = FillEvent {
                        fill_id,
                        price: best_price,
                        qty: fill_qty,
                        maker_user_id: resting_order.user_id.to_owned(),
                        maker_order_id: resting_order.order_id.to_owned(),
                        taker_user_id: user_id.to_owned(),
                        taker_side: side,
                        is_maker_fully_filled: false,
                        maker_total_qty: resting_order.qty,
                    };

                    resting_order.filled_qty += fill_qty;
                    remaining_qty -= fill_qty;
                    if fill_qty == available_qty {
                        fill_event.is_maker_fully_filled = true;
                        orders_at_price.pop_front();
                    }

                    fills.push(fill_event);
                }
            }

            if orders_at_price.is_empty() {
                match side {
                    OrderSide::Buy => self.asks.remove(&best_price),
                    OrderSide::Sell => self.bids.remove(&best_price),
                };
            }

            // end to update to the best next price
            best_next_price = match side {
                OrderSide::Buy => self.get_next_best_ask_price(Some(best_price)),
                OrderSide::Sell => self.get_next_best_bid_price(Some(best_price)),
            };
        }

        let filled_qty = qty - remaining_qty;
        let taker_final_status: OrderStatus;
        if filled_qty == dec!(0) {
            taker_final_status = OrderStatus::Open;
        } else if qty > filled_qty {
            taker_final_status = OrderStatus::PartialFilled;
        } else {
            taker_final_status = OrderStatus::Filled;
        }

        MatchResult {
            fills,
            filled_qty,
            remaining_qty,
            requested_price: price,
            taker_final_status,
        }
    }
}

#[derive(Debug)]
pub struct Store {
    pub balances: Balances,
    pub orderbooks: HashMap<MarketId, OrderBook>,
    pub orders: HashMap<OrderId, OrderRecord>,
    pub fills: Vec<Fill>,
}
impl Store {
    pub fn record_match(
        &mut self,
        taker_user_id: &str,
        taker_order_id: String,
        symbol: &str,
        side: OrderSide,
        order_type: OrderType,
        total_qty: Decimal,
        match_result: &MatchResult,
    ) {
        let MatchResult {
            fills,
            filled_qty,
            requested_price,
            taker_final_status,
            ..
        } = match_result;

        let mut taker_record = OrderRecord {
            order_id: taker_order_id,
            user_id: taker_user_id.to_string(),
            side: side.clone(),
            order_type,
            symbol: symbol.to_string(),
            price: Some(requested_price.clone()),
            qty: total_qty,
            filled_qty: filled_qty.to_owned(),
            status: taker_final_status.clone(),
            fills: vec![],
            created_at: Utc::now(),
        };

        for event in fills {
            let (buy_order_id, buy_user_id, sell_order_id, sell_user_id, maker_side) =
                match event.taker_side {
                    OrderSide::Buy => (
                        taker_record.order_id.clone(),
                        event.taker_user_id.clone(),
                        event.maker_order_id.clone(),
                        event.maker_user_id.clone(),
                        OrderSide::Sell,
                    ),
                    OrderSide::Sell => (
                        event.maker_order_id.clone(),
                        event.maker_user_id.clone(),
                        taker_record.order_id.clone(),
                        event.taker_user_id.clone(),
                        OrderSide::Buy,
                    ),
                };
            let fill = Fill {
                fill_id: event.fill_id.to_owned(),
                symbol: symbol.to_owned(),
                price: event.price,
                qty: event.qty,
                buy_order_id,
                buy_user_id,
                sell_order_id,
                sell_user_id,
                created_at: Utc::now(),
            };
            taker_record.fills.push(fill.clone());

            let resting_order_record = self
                .orders
                .entry(event.maker_order_id.clone())
                .or_insert_with(|| OrderRecord {
                    order_id: event.maker_order_id.clone(),
                    user_id: event.maker_user_id.clone(),
                    side: maker_side,
                    order_type: OrderType::Limit,
                    symbol: symbol.to_owned(),
                    price: Some(event.price),
                    qty: event.maker_total_qty,
                    filled_qty: dec!(0),
                    status: if event.is_maker_fully_filled {
                        OrderStatus::Filled
                    } else {
                        OrderStatus::PartialFilled
                    },
                    fills: vec![],
                    created_at: Utc::now(),
                });
            resting_order_record.filled_qty += event.qty;
            resting_order_record.fills.push(fill.clone());
            resting_order_record.status = if event.is_maker_fully_filled {
                OrderStatus::Filled
            } else {
                OrderStatus::PartialFilled
            };

            self.fills.push(fill);
        }

        self.orders
            .insert(taker_record.order_id.clone(), taker_record);
    }
}

pub fn create_exchange_store() -> Store {
    let sol_orderbook = (
        "SOL".to_string(),
        OrderBook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        },
    );
    let btc_orderbook = (
        "BTC".to_string(),
        OrderBook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        },
    );
    return Store {
        balances: Balances {
            accounts: HashMap::new(),
        },
        orderbooks: HashMap::from([sol_orderbook, btc_orderbook]),
        orders: HashMap::new(),
        fills: Vec::new(),
    };
}
