use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueueRequest {
    InitUserBalance {
        #[serde(rename = "correlationId")]
        correlation_id: String,
        #[serde(rename = "responseQueue")]
        response_queue: String,
        payload: Value,
    },
    CreateOrder {
        #[serde(rename = "correlationId")]
        correlation_id: String,
        #[serde(rename = "responseQueue")]
        response_queue: String,
        payload: CreateOrderPayload,
    },
    GetDepth {
        #[serde(rename = "correlationId")]
        correlation_id: String,
        #[serde(rename = "responseQueue")]
        response_queue: String,
        payload: Value,
    },
    GetUserBalance {
        #[serde(rename = "correlationId")]
        correlation_id: String,
        #[serde(rename = "responseQueue")]
        response_queue: String,
        payload: Value,
    },
    GetOrder {
        #[serde(rename = "correlationId")]
        correlation_id: String,
        #[serde(rename = "responseQueue")]
        response_queue: String,
        payload: Value,
    },
    CancelOrder {
        #[serde(rename = "correlationId")]
        correlation_id: String,
        #[serde(rename = "responseQueue")]
        response_queue: String,
        payload: Value,
    },
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderPayload {
    pub price: f64,
    pub qty: f64,
    pub side: OrderSide,
    pub symbol: String,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    #[serde(rename = "userId")]
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueResponse<T> {
    pub correlation_id: String,
    pub ok: bool,
    pub data: T,
}

impl QueueRequest {
    pub fn correlation_id(&self) -> &str {
        match self {
            Self::InitUserBalance { correlation_id, .. }
            | Self::CreateOrder { correlation_id, .. }
            | Self::GetDepth { correlation_id, .. }
            | Self::GetUserBalance { correlation_id, .. }
            | Self::GetOrder { correlation_id, .. }
            | Self::CancelOrder { correlation_id, .. } => correlation_id,
        }
    }

    pub fn response_queue(&self) -> &str {
        match self {
            Self::InitUserBalance { response_queue, .. }
            | Self::CreateOrder { response_queue, .. }
            | Self::GetDepth { response_queue, .. }
            | Self::GetUserBalance { response_queue, .. }
            | Self::GetOrder { response_queue, .. }
            | Self::CancelOrder { response_queue, .. } => response_queue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_create_order_request() {
        let payload = r#"{
          "correlationId": "0a9e3020-d0e3-47af-b6ab-6eff55924550",
          "payload": {
            "price": 9,
            "qty": 9,
            "side": "buy",
            "symbol": "BTC",
            "type": "limit",
            "userId": "0111bb09-3bd5-4551-b35a-b6c7d9e1c350"
          },
          "responseQueue": "response-queue-2219e38e-d908-45ba-9282-59425539852b",
          "type": "create_order"
        }"#;

        let request = serde_json::from_str::<QueueRequest>(payload).unwrap();

        assert_eq!(
            request.correlation_id(),
            "0a9e3020-d0e3-47af-b6ab-6eff55924550"
        );
        assert_eq!(
            request.response_queue(),
            "response-queue-2219e38e-d908-45ba-9282-59425539852b"
        );

        let QueueRequest::CreateOrder { payload, .. } = request else {
            panic!("expected create_order request");
        };

        assert_eq!(payload.symbol, "BTC");
        assert_eq!(payload.user_id, "0111bb09-3bd5-4551-b35a-b6c7d9e1c350");
    }
}
