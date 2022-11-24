use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebRequest {
    Request,
    RequestWithPayload(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebEvent {
    Event,
    EventWithPayload(u32),
    MalformedRequest,
}
