use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use rand::RngCore;
use serde_json::json;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn invalid(path: &str, reason: &str) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_request",
            format!("{path}: {reason}"),
        )
    }

    pub fn response(self, openrouter: bool) -> Response {
        let body = if openrouter {
            json!({"error":{"code":self.status.as_u16(),"message":self.message},"id":random_uuid()})
        } else {
            json!({"error":{"code":self.code,"message":self.message}})
        };
        (self.status, Json(body)).into_response()
    }
}

/// A random RFC 9562 version 4 identifier. The coin has paperwork, too.
fn random_uuid() -> String {
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut id = String::with_capacity(36);
    for (index, byte) in bytes.into_iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            id.push('-');
        }
        id.push(char::from(HEX[usize::from(byte >> 4)]));
        id.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    id
}
