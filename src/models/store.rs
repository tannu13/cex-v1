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
    pub sell_order_id: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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
}

pub struct FillEvent {
    pub fill_id: String,
    pub price: Decimal,
    pub qty: Decimal,
    pub maker_user_id: String,
    pub taker_user_id: String,
    pub taker_side: OrderSide,
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
    pub fn match_limit_buy(&mut self, price: Decimal, qty: Decimal) -> MatchResult {
        let mut remaining_qty = qty;
        let mut fills = Vec::new();
        let mut best_next_price = self.get_next_best_ask_price(None);

        while let Some(best_price) = best_next_price {
            if remaining_qty <= dec!(0) || best_price > price {
                break;
            }

            let mut remove_price_level = false;
            let orders_at_price = self
                .asks
                .get_mut(&best_price)
                .expect("order at best price was validated earlier");
            while remaining_qty > dec!(0) {
                let mut remove_front_order = false;
                if let Some(resting_order) = orders_at_price.front_mut() {
                    let available_qty = resting_order.qty - resting_order.filled_qty;

                    let fill_id = Uuid::new_v4().to_string();
                    let fill_qty = min(available_qty, remaining_qty);
                    fills.push(FillEvent {
                        fill_id,
                        price: best_price,
                        qty: fill_qty,
                        maker_user_id: resting_order.user_id.to_owned(),
                        taker_user_id: resting_order.order_id.to_owned(),
                        taker_side: OrderSide::Buy,
                    });
                    if available_qty > remaining_qty {
                        remaining_qty = dec!(0);
                        break;
                    } else if available_qty == remaining_qty {
                        remaining_qty = dec!(0);
                        remove_front_order = true;
                    } else {
                        // available_qty < remaining_qty
                        remaining_qty -= available_qty;
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
                self.asks.remove(&best_price);
            }

            // end to update to the best next price
            best_next_price = self.get_next_best_ask_price(Some(best_price));
        }

        let filled_qty = qty - remaining_qty;

        MatchResult {
            fills,
            filled_qty,
            remaining_qty,
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
