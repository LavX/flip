use std::{
    error::Error,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use async_compression::tokio::bufread::{GzipDecoder, ZstdDecoder};
use axum::{
    body::{to_bytes, Body, Bytes},
    extract::{DefaultBodyLimit, FromRequest, Multipart, Request, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rand::{rngs::StdRng, SeedableRng};
use serde_json::{json, Map, Value};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, ReadBuf};
use tower_http::cors::CorsLayer;

use crate::{
    decide,
    error::ApiError,
    models::{MAX_BODY_BYTES, MODEL},
    validate,
};

#[derive(Clone, Copy)]
struct ApiState {
    allow_test_seed: bool,
}

/// Build the API. Test seeds require explicit opt-in, even for banana owners.
pub fn router() -> Router {
    router_with_test_seed(std::env::var("FLIP_ALLOW_TEST_SEED").as_deref() == Ok("true"))
}

/// Set the seed policy without changing process environment during concurrent tests.
pub fn router_with_test_seed(allow_test_seed: bool) -> Router {
    let api = Router::new()
        .route("/v1/systemone", post(decisions))
        .route("/api/alpha/decisions", post(decisions))
        .route("/v1/models", get(models))
        .layer(DefaultBodyLimit::disable())
        .with_state(ApiState { allow_test_seed });
    api.route("/health", get(health))
        .layer(CorsLayer::permissive())
        .merge(crate::static_files::router())
}

async fn health() -> Json<Value> {
    Json(json!({"status":"ok", "model":MODEL}))
}

async fn models(headers: HeaderMap) -> Response {
    match authenticate(&headers) {
        Ok(()) => Json(crate::models::catalog()).into_response(),
        Err(error) => error.response(false),
    }
}

/// Both compatibility routes make the same decisions. Only errors wear a disguise.
async fn decisions(State(state): State<ApiState>, request: Request) -> Response {
    let openrouter = request.uri().path() == "/api/alpha/decisions";
    match process_request(state, request).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => error.response(openrouter),
    }
}

async fn process_request(state: ApiState, request: Request) -> Result<Value, ApiError> {
    authenticate(request.headers())?;
    let (parts, body) = request.into_parts();
    // Count the actual wire bytes, including multipart framing and files. Content-Length
    // is neither required nor trusted, so chunked requests have the same limit.
    let wire = to_bytes(body, MAX_BODY_BYTES)
        .await
        .map_err(body_read_error)?;
    let seed = state
        .allow_test_seed
        .then(|| {
            parts
                .headers
                .get("x-flip-seed")?
                .to_str()
                .ok()?
                .parse::<u64>()
                .ok()
        })
        .flatten();
    let coding_headers = parts.headers.clone();
    let decoded = decode_body(wire, &coding_headers).await?;
    let decoded = if is_multipart(&parts.headers) {
        multipart_json(Request::from_parts(parts, Body::from(decoded))).await?
    } else {
        Bytes::from(decoded)
    };
    let request = validate::parse_request(&decoded)?;

    if request.model == "flip-overloaded" {
        return Err(ApiError::new(
            StatusCode::from_u16(529).expect("529 is a valid HTTP status"),
            "overloaded",
            "The coin is fine. This alias exists so you can test retry logic.",
        ));
    }

    let force_edge = request.model == "flip-edge";
    let mut answers = Map::new();
    let mut rng = rand::thread_rng();
    for (id, question) in &request.questions {
        let answer = match seed {
            Some(seed) => {
                let mut question_rng = StdRng::seed_from_u64(question_seed(seed, id));
                decide::answer(question, force_edge, &mut question_rng)
            }
            None => decide::answer(question, force_edge, &mut rng),
        };
        answers.insert(id.clone(), answer);
    }

    Ok(json!({
        "model":MODEL,
        "answers":answers,
        "usage":{
            "input_tokens":synthetic_input_tokens(decoded.len()),
            "output_tokens":request.questions.len(),
            "cost":0.0
        }
    }))
}

/// A synthetic token estimate from decoded JSON bytes, not an actual tokenizer.
/// The coin charges nothing and still insists on an invoice.
fn synthetic_input_tokens(bytes: usize) -> usize {
    bytes.div_ceil(4).max(1)
}

/// Stable FNV-1a over a seed and question id makes answers independent of question order.
fn question_seed(seed: u64, id: &str) -> u64 {
    seed.to_le_bytes()
        .iter()
        .chain(id.as_bytes())
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

/// We accept banana. The token must still show up for work.
fn authenticate(headers: &HeaderMap) -> Result<(), ApiError> {
    if headers
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.split_once(' '))
        .is_some_and(|(scheme, token)| {
            scheme.eq_ignore_ascii_case("Bearer") && !token.trim().is_empty()
        })
    {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "missing_bearer",
            "Authorization must contain Bearer followed by a non-empty token; we accept banana.",
        ))
    }
}

fn payload_too_large() -> ApiError {
    ApiError::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload_too_large",
        "request body exceeds the 33,554,432 byte limit",
    )
}

fn body_read_error(error: axum::Error) -> ApiError {
    let mut source: &(dyn Error + 'static) = &error;
    loop {
        // axum wraps http-body-util's LengthLimitError without re-exporting its type.
        if source.to_string() == "length limit exceeded" {
            return payload_too_large();
        }
        match source.source() {
            Some(next) => source = next,
            None => break,
        }
    }
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "malformed_json",
        "request body could not be read",
    )
}

fn is_multipart(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|header| header.to_str().ok())
        .is_some_and(|value| {
            value.split(';').next().is_some_and(|media_type| {
                media_type
                    .trim()
                    .eq_ignore_ascii_case("multipart/form-data")
            })
        })
}

async fn multipart_json(request: Request) -> Result<Bytes, ApiError> {
    let mut multipart = Multipart::from_request(request, &())
        .await
        .map_err(|_| multipart_error("request: invalid multipart boundary"))?;
    let mut json_body = None;
    let mut part_bytes = 0_usize;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| multipart_error("request: invalid multipart data"))?
    {
        let is_request = field.name() == Some("request");
        if is_request && json_body.is_some() {
            return Err(multipart_error("request: duplicate multipart JSON part"));
        }
        let mut content = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|_| multipart_error("request: could not read multipart part"))?
        {
            part_bytes += chunk.len();
            if part_bytes > MAX_BODY_BYTES {
                return Err(payload_too_large());
            }
            if is_request {
                content.extend_from_slice(&chunk);
            }
            // All other parts, including uploaded files, are stream-discarded.
        }
        if is_request {
            json_body = Some(Bytes::from(content));
        }
    }
    json_body.ok_or_else(|| multipart_error("request: missing multipart JSON part"))
}

fn multipart_error(message: &str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "malformed_json", message)
}

async fn decode_body(bytes: Bytes, headers: &HeaderMap) -> Result<Vec<u8>, ApiError> {
    let mut codings = Vec::new();
    for value in headers.get_all(header::CONTENT_ENCODING) {
        let value = value.to_str().map_err(|_| unsupported_encoding())?;
        codings.extend(
            value
                .split(',')
                .map(|coding| coding.trim().to_ascii_lowercase()),
        );
    }
    if codings.is_empty() {
        return Ok(bytes.to_vec());
    }
    if codings.len() != 1 {
        return Err(unsupported_encoding());
    }
    if !matches!(codings[0].as_str(), "gzip" | "x-gzip" | "zstd") {
        return Err(unsupported_encoding());
    }
    if bytes.is_empty() {
        return Err(decompression_failed());
    }
    match codings[0].as_str() {
        "gzip" | "x-gzip" => {
            let mut decoder = GzipDecoder::new(bytes.as_ref());
            decoder.multiple_members(true);
            read_decoded(decoder).await
        }
        "zstd" => {
            // async-compression does not expose the real API's 8 MiB zstd window rule.
            // Approximate it with the strict 32 MiB decoded-size bound below.
            let mut remaining = bytes.as_ref();
            let mut decoded = Vec::new();
            while !remaining.is_empty() {
                // In 0.4.18 a zstd decoder can report success on premature EOF.
                // A complete single frame stops before polling EOF. Turn an EOF
                // poll into an error, then continue explicitly with any next frame.
                let mut decoder = ZstdDecoder::new(CompleteFrameInput { remaining });
                read_decoded_into(&mut decoder, &mut decoded).await?;
                remaining = decoder.into_inner().remaining;
            }
            Ok(decoded)
        }
        _ => unreachable!("the coding was checked above"),
    }
}

async fn read_decoded(reader: impl AsyncRead + Unpin) -> Result<Vec<u8>, ApiError> {
    let mut decoded = Vec::new();
    read_decoded_into(reader, &mut decoded).await?;
    Ok(decoded)
}

async fn read_decoded_into(
    reader: impl AsyncRead + Unpin,
    decoded: &mut Vec<u8>,
) -> Result<(), ApiError> {
    let result = reader
        .take((MAX_BODY_BYTES - decoded.len()) as u64 + 1)
        .read_to_end(decoded)
        .await;
    if decoded.len() > MAX_BODY_BYTES {
        return Err(payload_too_large());
    }
    result.map_err(|_| decompression_failed())?;
    Ok(())
}

/// A complete zstd frame must finish before asking its input for another byte.
struct CompleteFrameInput<'a> {
    remaining: &'a [u8],
}

impl AsyncRead for CompleteFrameInput<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buffer.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        if self.remaining.is_empty() {
            return Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()));
        }
        let count = buffer.remaining().min(self.remaining.len());
        buffer.put_slice(&self.remaining[..count]);
        self.remaining = &self.remaining[count..];
        Poll::Ready(Ok(()))
    }
}

impl AsyncBufRead for CompleteFrameInput<'_> {
    fn poll_fill_buf(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let remaining = self.get_mut().remaining;
        if remaining.is_empty() {
            Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()))
        } else {
            Poll::Ready(Ok(remaining))
        }
    }

    fn consume(mut self: Pin<&mut Self>, amount: usize) {
        self.remaining = &self.remaining[amount..];
    }
}

fn unsupported_encoding() -> ApiError {
    ApiError::new(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_content_encoding",
        "Content-Encoding must contain one of gzip, x-gzip, or zstd",
    )
}

fn decompression_failed() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "decompression_failed",
        "request body could not be decompressed: corrupt or truncated stream",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn auth_precedes_body_parsing() {
        let response = router()
            .oneshot(
                Request::post("/v1/systemone")
                    .body(Body::from("bad json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn zstd_requires_complete_frames_including_the_last_byte() {
        use async_compression::tokio::write::ZstdEncoder;
        use tokio::io::AsyncWriteExt;

        let body = br#"{"state":"x","model":"flip-latest","questions":{"q":{"type":"noul"}}}"#;
        let mut encoder = ZstdEncoder::new(Vec::new());
        encoder.write_all(body).await.unwrap();
        encoder.shutdown().await.unwrap();
        let compressed = encoder.into_inner();
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_ENCODING, "zstd".parse().unwrap());
        assert_eq!(
            decode_body(compressed.clone().into(), &headers)
                .await
                .unwrap(),
            body
        );
        for missing in [1, 5] {
            let truncated = compressed[..compressed.len() - missing].to_vec();
            let error = decode_body(truncated.into(), &headers).await.unwrap_err();
            assert_eq!(error.code, "decompression_failed");
        }

        let mut multiple_frames = Vec::new();
        for fragment in [&body[..20], &body[20..]] {
            let mut encoder = ZstdEncoder::new(Vec::new());
            encoder.write_all(fragment).await.unwrap();
            encoder.shutdown().await.unwrap();
            multiple_frames.extend(encoder.into_inner());
        }
        assert_eq!(
            decode_body(multiple_frames.clone().into(), &headers)
                .await
                .unwrap(),
            body
        );
        multiple_frames.pop();
        assert_eq!(
            decode_body(multiple_frames.into(), &headers)
                .await
                .unwrap_err()
                .code,
            "decompression_failed"
        );
    }
}
