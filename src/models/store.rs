use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

type UserId = String;
type MarketId = String;
type Currency = String;

#[derive(Debug)]
pub struct Balance {
    pub available: Decimal,
    pub locked: Decimal,
}
#[derive(Debug, Serialize, Deserialize)]
struct Fill {
    pub fill_id: String,
    pub symbol: String,
    pub price: Decimal,
    pub qty: Decimal,
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Filled,
    Open,
    PartiallFilled,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestingOrder {
    pub order_id: String,
    pub user_id: String,
    pub side: Side,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub symbol: String,
    pub price: Decimal,
    pub qty: Decimal,
    pub filled_qty: Decimal,
    pub status: OrderStatus,
    pub created_at: Decimal,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRecord {
    pub order_id: String,
    pub user_id: String,
    pub side: Side,
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
    pub bids: BTreeMap<Decimal, Vec<RestingOrder>>,
    pub asks: BTreeMap<Decimal, Vec<RestingOrder>>,
}

#[derive(Debug)]
pub struct Store {
    pub balances: HashMap<UserId, HashMap<Currency, Balance>>,
    pub orderbooks: HashMap<MarketId, OrderBook>,
    pub orders: HashMap<MarketId, OrderRecord>,
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
        balances: HashMap::new(),
        orderbooks: HashMap::from([sol_orderbook, btc_orderbook]),
        orders: HashMap::new(),
        fills: Vec::new(),
    };
}
