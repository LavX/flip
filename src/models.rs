use serde_json::{Map, Value};

pub const MODEL: &str = "flip-1.0.0";
pub const MAX_BODY_BYTES: usize = 33_554_432;

/// The validated parts the coin can actually understand.
#[derive(Debug)]
pub struct DecisionRequest {
    pub model: String,
    pub questions: Vec<(String, Question)>,
}

#[derive(Debug)]
pub enum Question {
    Noul,
    Choice(Map<String, Value>),
    Score(Vec<Value>),
    Box,
}

/// The aliases are aspirational. Every successful response still reports MODEL.
pub fn catalog() -> Value {
    serde_json::json!({"models":[
        {"name":"flip-latest","description":"The most recent stable release of the coin.","release_date":"2026-10-15"},
        {"name":"flip-preview","description":"Resolves to flip-1.1.0, the two-sided coin.","release_date":"2026-10-15"},
        {"name":"flip-edge","description":"Forces an edge landing on every flip. Confidence exactly 0.0.","release_date":"2026-10-15"},
        {"name":"flip-overloaded","description":"Returns 529 so you can test retry logic. The coin itself is fine.","release_date":"2026-10-15"}
    ]})
}
