use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

type UserId = String;
type MarketId = String;
type Currency = String;

#[derive(Debug)]
struct Balance {
    available: Decimal,
    locked: Decimal,
}
#[derive(Debug, Serialize, Deserialize)]
struct Fill {
    fill_id: String,
    symbol: String,
    price: Decimal,
    qty: Decimal,
    buy_order_id: String,
    sell_order_id: String,
    created_at: DateTime<Utc>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Buy,
    Sell,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OrderStatus {
    Filled,
    Open,
    PartiallFilled,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestingOrder {
    order_id: String,
    user_id: String,
    side: Side,
    #[serde(rename = "type")]
    order_type: OrderType,
    symbol: String,
    price: Decimal,
    qty: Decimal,
    filled_qty: Decimal,
    status: OrderStatus,
    created_at: Decimal,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderRecord {
    order_id: String,
    user_id: String,
    side: Side,
    #[serde(rename = "type")]
    order_type: OrderType,
    symbol: String,
    price: Option<Decimal>,
    qty: Decimal,
    filled_qty: Decimal,
    status: OrderStatus,
    fills: Vec<Fill>,
    created_at: DateTime<Utc>,
}

struct OrderBook {
    bids: BTreeMap<Decimal, Vec<RestingOrder>>,
    asks: BTreeMap<Decimal, Vec<RestingOrder>>,
}

struct Store {
    balances: HashMap<UserId, HashMap<Currency, Balance>>,
    orderbooks: HashMap<MarketId, OrderBook>,
    orders: HashMap<MarketId, OrderRecord>,
    fills: Vec<Fill>,
}
pub fn create_exchange_store() -> Store {
    return Store {
        balances: HashMap::new(),
        orderbooks: HashMap::new(),
        orders: HashMap::new(),
        fills: Vec::new(),
    };
}
