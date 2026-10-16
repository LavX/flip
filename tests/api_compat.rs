use async_compression::tokio::write::{GzipEncoder, ZstdEncoder};
use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderName, HeaderValue, Method, Request, StatusCode},
    response::Response,
    Router,
};
use flip::api::{router, router_with_test_seed};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tower::ServiceExt;

const OPENROUTER: &str = "/api/alpha/decisions";
const TYPESAFE: &str = "/v1/systemone";
const MAX_BODY_BYTES: usize = 33_554_432;
const AUTH: (&str, &str) = ("authorization", "Bearer banana");

async fn call(
    app: Router,
    method: Method,
    path: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::from(body))
        .expect("request builds");
    for (name, value) in headers {
        request.headers_mut().append(
            HeaderName::from_bytes(name.as_bytes()).expect("valid header name"),
            HeaderValue::from_str(value).expect("valid header value"),
        );
    }
    app.oneshot(request).await.expect("router response")
}

async fn response_json(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body is readable");
    let value = serde_json::from_slice(&bytes).expect("response body is JSON");
    (status, value)
}

fn valid_request() -> Value {
    json!({
        "state": "x",
        "model": "flip-latest",
        "questions": {"q": {"type": "noul"}}
    })
}

fn body_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("request serializes")
}

async fn post_json(app: Router, path: &str, value: &Value) -> (StatusCode, Value) {
    response_json(
        call(
            app,
            Method::POST,
            path,
            &[
                (AUTH.0, AUTH.1),
                (header::CONTENT_TYPE.as_str(), "application/json"),
            ],
            body_bytes(value),
        )
        .await,
    )
    .await
}

async fn post_json_seeded(app: Router, path: &str, value: &Value) -> (StatusCode, Value) {
    response_json(
        call(
            app,
            Method::POST,
            path,
            &[
                (AUTH.0, AUTH.1),
                ("x-flip-seed", "1337"),
                (header::CONTENT_TYPE.as_str(), "application/json"),
            ],
            body_bytes(value),
        )
        .await,
    )
    .await
}

fn assert_uuid(value: &Value) {
    let id = value.as_str().expect("OpenRouter error has a string id");
    let pieces: Vec<_> = id.split('-').collect();
    assert_eq!(
        pieces.iter().map(|part| part.len()).collect::<Vec<_>>(),
        [8, 4, 4, 4, 12]
    );
    assert!(pieces
        .iter()
        .all(|part| part.chars().all(|ch| ch.is_ascii_hexdigit())));
    assert_eq!(pieces[2].as_bytes()[0], b'4', "id is a UUID v4");
    assert!(matches!(pieces[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
}

async fn assert_error_shape(
    path: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
    expected: StatusCode,
) {
    let (status, value) =
        response_json(call(router(), Method::POST, path, headers, body).await).await;
    assert_eq!(status, expected, "{path} error status");
    let error = value.get("error").expect("error envelope");
    assert!(error.get("message").and_then(Value::as_str).is_some());
    if path == TYPESAFE {
        assert!(error.get("code").and_then(Value::as_str).is_some());
        assert!(value.get("id").is_none());
    } else {
        assert_eq!(
            error.get("code").and_then(Value::as_u64),
            Some(expected.as_u16() as u64)
        );
        assert_uuid(value.get("id").expect("OpenRouter error id"));
    }
}

async fn assert_get_error_status(path: &str, headers: &[(&str, &str)], expected: StatusCode) {
    let status = call(router(), Method::GET, path, headers, Vec::new())
        .await
        .status();
    assert_eq!(status, expected, "GET {path} error status");
}

fn exact_size_valid_body(size: usize) -> Vec<u8> {
    let prefix = br#"{"state":""#;
    let suffix = br#"","model":"flip-latest","questions":{"q":{"type":"noul"}}}"#;
    assert!(size >= prefix.len() + suffix.len());
    let mut body = Vec::with_capacity(size);
    body.extend_from_slice(prefix);
    body.extend(std::iter::repeat_n(
        b'x',
        size - prefix.len() - suffix.len(),
    ));
    body.extend_from_slice(suffix);
    assert_eq!(body.len(), size);
    body
}

fn nested_arrays(depth: usize) -> String {
    format!("{}\"leaf\"{}", "[".repeat(depth), "]".repeat(depth))
}

fn raw_request_with_value(field: &str, value: &str) -> Vec<u8> {
    let mut request = valid_request();
    request[field] = serde_json::from_str(value).expect("valid JSON value");
    body_bytes(&request)
}

async fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzipEncoder::new(Vec::new());
    encoder.write_all(bytes).await.expect("gzip write");
    encoder.shutdown().await.expect("gzip finish");
    encoder.into_inner()
}

async fn zstd(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = ZstdEncoder::new(Vec::new());
    encoder.write_all(bytes).await.expect("zstd write");
    encoder.shutdown().await.expect("zstd finish");
    encoder.into_inner()
}

#[tokio::test]
async fn health_and_models_have_the_documented_shapes() {
    let (status, health) =
        response_json(call(router(), Method::GET, "/health", &[], Vec::new()).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health, json!({"status":"ok","model":"flip-1.0.0"}));

    let (status, models) =
        response_json(call(router(), Method::GET, "/v1/models", &[AUTH], Vec::new()).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        models,
        json!({
            "models": [
                {"name":"flip-latest","description":"The most recent stable release of the coin.","release_date":"2026-10-15"},
                {"name":"flip-preview","description":"Resolves to flip-1.1.0, the two-sided coin.","release_date":"2026-10-15"},
                {"name":"flip-edge","description":"Forces an edge landing on every flip. Confidence exactly 0.0.","release_date":"2026-10-15"},
                {"name":"flip-overloaded","description":"Returns 529 so you can test retry logic. The coin itself is fine.","release_date":"2026-10-15"}
            ]
        })
    );
}

#[tokio::test]
async fn health_is_public_and_models_require_bearer_auth() {
    assert_get_error_status("/v1/models", &[], StatusCode::UNAUTHORIZED).await;
    assert_error_shape(
        OPENROUTER,
        &[(header::CONTENT_TYPE.as_str(), "application/json")],
        body_bytes(&valid_request()),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    assert_error_shape(
        TYPESAFE,
        &[
            ("authorization", "Basic banana"),
            (header::CONTENT_TYPE.as_str(), "application/json"),
        ],
        body_bytes(&valid_request()),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    assert_error_shape(
        OPENROUTER,
        &[
            ("authorization", "Bearer "),
            (header::CONTENT_TYPE.as_str(), "application/json"),
        ],
        body_bytes(&valid_request()),
        StatusCode::UNAUTHORIZED,
    )
    .await;

    let (status, _) = post_json(router(), OPENROUTER, &valid_request()).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "any nonempty Bearer token is accepted"
    );
}

#[tokio::test]
async fn both_decision_routes_return_the_same_success_answers() {
    let request = valid_request();
    let app = router_with_test_seed(true);
    let (type_safe_status, type_safe) = post_json_seeded(app.clone(), TYPESAFE, &request).await;
    let (openrouter_status, openrouter) = post_json_seeded(app, OPENROUTER, &request).await;
    assert_eq!(type_safe_status, StatusCode::OK);
    assert_eq!(openrouter_status, StatusCode::OK);
    assert_eq!(type_safe["model"], "flip-1.0.0");
    assert_eq!(type_safe["answers"], openrouter["answers"]);
    assert_eq!(type_safe["answers"]["q"]["type"], "noul");
    assert_eq!(type_safe["usage"]["input_tokens"], 18);
    assert_eq!(type_safe["usage"]["output_tokens"], 1);
    assert_eq!(type_safe["usage"]["cost"], 0.0);
}

#[tokio::test]
async fn bearer_scheme_is_case_insensitive_and_empty_tokens_are_rejected() {
    for (path, authorization, expected) in [
        (OPENROUTER, "bearer banana", StatusCode::OK),
        (TYPESAFE, "bEaReR banana", StatusCode::OK),
        (TYPESAFE, "Bearer ", StatusCode::UNAUTHORIZED),
    ] {
        let (status, _) = response_json(
            call(
                router(),
                Method::POST,
                path,
                &[
                    (AUTH.0, authorization),
                    (header::CONTENT_TYPE.as_str(), "application/json"),
                ],
                body_bytes(&valid_request()),
            )
            .await,
        )
        .await;
        assert_eq!(
            status, expected,
            "authorization {authorization:?} on {path}"
        );
    }
}

#[tokio::test]
async fn cors_preflight_does_not_require_authentication() {
    let response = call(
        router(),
        Method::OPTIONS,
        OPENROUTER,
        &[
            (header::ORIGIN.as_str(), "https://client.example"),
            ("access-control-request-method", "POST"),
            (
                "access-control-request-headers",
                "authorization,content-type",
            ),
        ],
        Vec::new(),
    )
    .await;
    assert!(response.status().is_success());
    assert!(response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    assert!(response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_METHODS));
    assert!(response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_HEADERS));
}

#[tokio::test]
async fn parse_failures_use_the_two_error_envelopes() {
    let cases = [
        (b"{".to_vec(), "malformed_json"),
        (vec![0xff], "invalid_utf8"),
        (
            {
                let mut bom = vec![0xef, 0xbb, 0xbf];
                bom.extend_from_slice(
                    br#"{"state":"x","model":"flip-latest","questions":{"q":{"type":"noul"}}}"#,
                );
                bom
            },
            "byte_order_mark",
        ),
        (
            br#"{"state":1e400,"model":"flip-latest","questions":{"q":{"type":"noul"}}}"#.to_vec(),
            "non_finite_number",
        ),
    ];
    for (body, code) in cases {
        for path in [OPENROUTER, TYPESAFE] {
            assert_error_shape(
                path,
                &[
                    (AUTH.0, AUTH.1),
                    (header::CONTENT_TYPE.as_str(), "application/json"),
                ],
                body.clone(),
                StatusCode::BAD_REQUEST,
            )
            .await;
        }
        let (status, type_safe) =
            response_json(call(router(), Method::POST, TYPESAFE, &[(AUTH.0, AUTH.1)], body).await)
                .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(type_safe["error"]["code"], code);
    }
}

#[tokio::test]
async fn request_schema_rejects_missing_null_and_out_of_range_fields() {
    let mut cases: Vec<Value> = Vec::new();
    let mut value = valid_request();
    value.as_object_mut().unwrap().remove("state");
    cases.push(value);
    let mut value = valid_request();
    value["state"] = Value::Null;
    cases.push(value);
    let mut value = valid_request();
    value.as_object_mut().unwrap().remove("model");
    cases.push(value);
    let mut value = valid_request();
    value["model"] = json!("");
    cases.push(value);
    let mut value = valid_request();
    value.as_object_mut().unwrap().remove("questions");
    cases.push(value);
    let mut value = valid_request();
    value["questions"] = json!({});
    cases.push(value);
    let mut value = valid_request();
    value["questions"]["q"] = json!({"type":"future"});
    cases.push(value);
    let mut value = valid_request();
    value["questions"]["q"] = json!({"type":"choice","criteria":{}});
    cases.push(value);
    let criteria = (0..256)
        .map(|index| (format!("choice-{index}"), Value::Null))
        .collect::<serde_json::Map<_, _>>();
    let mut value = valid_request();
    value["questions"]["q"] = json!({"type":"choice","criteria":criteria});
    cases.push(value);
    let mut value = valid_request();
    value["questions"]["q"] = json!({"type":"score","criteria":["only"]});
    cases.push(value);
    let mut value = valid_request();
    value["questions"]["q"] = json!({"type":"score","criteria":[0,1,2,3,4,5,6,7,8,9,10]});
    cases.push(value);

    for value in cases {
        let body = body_bytes(&value);
        for path in [OPENROUTER, TYPESAFE] {
            assert_error_shape(
                path,
                &[
                    (AUTH.0, AUTH.1),
                    (header::CONTENT_TYPE.as_str(), "application/json"),
                ],
                body.clone(),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
        }
    }
}

#[tokio::test]
async fn nesting_limit_is_64_for_state_instructions_and_criteria() {
    let state_64 = raw_request_with_value("state", &nested_arrays(64));
    let state_65 = raw_request_with_value("state", &nested_arrays(65));
    let mut instructions_64 = valid_request();
    instructions_64["questions"]["q"]["instructions"] =
        json!(serde_json::from_str::<Value>(&nested_arrays(64)).unwrap());
    let mut instructions_65 = valid_request();
    instructions_65["questions"]["q"]["instructions"] =
        json!(serde_json::from_str::<Value>(&nested_arrays(65)).unwrap());
    let mut criteria_64 = valid_request();
    criteria_64["questions"]["q"] = json!({"type":"noul","criteria":{"true":serde_json::from_str::<Value>(&nested_arrays(63)).unwrap()}});
    let mut criteria_65 = valid_request();
    criteria_65["questions"]["q"] = json!({"type":"noul","criteria":{"true":serde_json::from_str::<Value>(&nested_arrays(64)).unwrap()}});

    for accepted in [
        state_64,
        body_bytes(&instructions_64),
        body_bytes(&criteria_64),
    ] {
        let (status, _) =
            response_json(call(router(), Method::POST, OPENROUTER, &[AUTH], accepted).await).await;
        assert_eq!(status, StatusCode::OK, "64 nested containers are accepted");
    }
    for rejected in [
        state_65,
        body_bytes(&instructions_65),
        body_bytes(&criteria_65),
    ] {
        for path in [OPENROUTER, TYPESAFE] {
            assert_error_shape(path, &[AUTH], rejected.clone(), StatusCode::BAD_REQUEST).await;
        }
        let (status, value) =
            response_json(call(router(), Method::POST, TYPESAFE, &[AUTH], rejected).await).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(value["error"]["code"], "nesting_too_deep");
    }
}

#[tokio::test]
async fn the_wire_body_limit_accepts_exactly_32_mib_and_rejects_one_byte_more() {
    let exact = exact_size_valid_body(MAX_BODY_BYTES);
    let (status, _) =
        response_json(call(router(), Method::POST, OPENROUTER, &[AUTH], exact).await).await;
    assert_eq!(status, StatusCode::OK);

    let too_large = exact_size_valid_body(MAX_BODY_BYTES + 1);
    let (status, _) =
        response_json(call(router(), Method::POST, OPENROUTER, &[AUTH], too_large).await).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn streamed_bodies_are_counted_without_a_content_length() {
    for (size, expected) in [
        (MAX_BODY_BYTES, StatusCode::OK),
        (MAX_BODY_BYTES + 1, StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        let bytes = axum::body::Bytes::from(exact_size_valid_body(size));
        let chunks = (0..bytes.len())
            .step_by(8192)
            .map(|start| {
                Ok::<_, std::convert::Infallible>(
                    bytes.slice(start..(start + 8192).min(bytes.len())),
                )
            })
            .collect::<Vec<_>>();
        let body = Body::from_stream(futures_util::stream::iter(chunks));
        let request = Request::post(TYPESAFE)
            .header(AUTH.0, AUTH.1)
            .header(header::TRANSFER_ENCODING, "chunked")
            .body(body)
            .unwrap();
        assert!(!request.headers().contains_key(header::CONTENT_LENGTH));
        let response = router().oneshot(request).await.unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn authentication_precedes_size_encoding_and_schema_checks() {
    let invalid_schema = json!({"state":null,"model":"","questions":{}});
    for (headers, body) in [
        (
            vec![(header::CONTENT_TYPE.as_str(), "application/json")],
            body_bytes(&invalid_schema),
        ),
        (
            vec![(header::CONTENT_ENCODING.as_str(), "br")],
            body_bytes(&valid_request()),
        ),
        (vec![], exact_size_valid_body(MAX_BODY_BYTES + 1)),
    ] {
        let refs = headers.to_vec();
        for path in [OPENROUTER, TYPESAFE] {
            let (status, _) =
                response_json(call(router(), Method::POST, path, &refs, body.clone()).await).await;
            assert_eq!(
                status,
                StatusCode::UNAUTHORIZED,
                "auth precedes other checks on {path}"
            );
        }
    }
}

#[tokio::test]
async fn wire_limit_is_checked_before_content_encoding() {
    let (status, _) = response_json(
        call(
            router(),
            Method::POST,
            OPENROUTER,
            &[(AUTH.0, AUTH.1), (header::CONTENT_ENCODING.as_str(), "br")],
            exact_size_valid_body(MAX_BODY_BYTES + 1),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn gzip_x_gzip_and_zstd_requests_are_decoded() {
    let body = body_bytes(&valid_request());
    for encoding in ["gzip", "x-gzip"] {
        let compressed = gzip(&body).await;
        let (status, value) = response_json(
            call(
                router(),
                Method::POST,
                OPENROUTER,
                &[
                    (AUTH.0, AUTH.1),
                    (header::CONTENT_ENCODING.as_str(), encoding),
                ],
                compressed,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{encoding} request is accepted");
        assert_eq!(value["answers"]["q"]["type"], "noul");
        assert_eq!(value["usage"]["input_tokens"], 18);
    }

    let compressed = zstd(&body).await;
    let (status, value) = response_json(
        call(
            router(),
            Method::POST,
            OPENROUTER,
            &[
                (AUTH.0, AUTH.1),
                (header::CONTENT_ENCODING.as_str(), "zstd"),
            ],
            compressed,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["answers"]["q"]["type"], "noul");
    assert_eq!(value["usage"]["input_tokens"], 18);
}

#[tokio::test]
async fn compressed_multipart_is_decoded_before_parsing_and_keeps_size_checks() {
    let boundary = "flip-compressed-boundary";
    let json_part = body_bytes(&valid_request());
    let multipart = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"request\"\r\nContent-Type: application/json\r\n\r\n{}\r\n--{boundary}--\r\n",
        String::from_utf8(json_part).unwrap()
    )
    .into_bytes();
    let headers = [
        AUTH,
        (
            header::CONTENT_TYPE.as_str(),
            "multipart/form-data; boundary=flip-compressed-boundary",
        ),
        (header::CONTENT_ENCODING.as_str(), "gzip"),
    ];

    for path in [OPENROUTER, TYPESAFE] {
        let (status, response) = response_json(
            call(
                router(),
                Method::POST,
                path,
                &headers,
                gzip(&multipart).await,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "compressed multipart on {path}");
        assert_eq!(response["answers"]["q"]["type"], "noul");
        assert_eq!(response["usage"]["input_tokens"], 18);
    }

    let mut corrupt = gzip(&multipart).await;
    corrupt.truncate(corrupt.len().saturating_sub(5));
    let (status, response) =
        response_json(call(router(), Method::POST, TYPESAFE, &headers, corrupt).await).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"]["code"], "decompression_failed");

    let mut oversized_multipart = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"request\"\r\nContent-Type: application/json\r\n\r\n"
    )
    .into_bytes();
    oversized_multipart.extend_from_slice(&exact_size_valid_body(MAX_BODY_BYTES + 1));
    oversized_multipart.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let (status, _) = response_json(
        call(
            router(),
            Method::POST,
            TYPESAFE,
            &headers,
            gzip(&oversized_multipart).await,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn multipart_files_above_axums_default_limit_are_discarded() {
    let mut body = b"--flip-large-file\r\nContent-Disposition: form-data; name=\"image\"; filename=\"photo.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
    body.resize(body.len() + 3 * 1024 * 1024, b'x');
    body.extend_from_slice(
        b"\r\n--flip-large-file\r\nContent-Disposition: form-data; name=\"request\"\r\n\r\n",
    );
    body.extend_from_slice(&body_bytes(&valid_request()));
    body.extend_from_slice(b"\r\n--flip-large-file--\r\n");
    let (status, response) = response_json(
        call(
            router(),
            Method::POST,
            TYPESAFE,
            &[
                AUTH,
                (
                    "content-type",
                    "multipart/form-data; boundary=flip-large-file",
                ),
            ],
            body,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["usage"]["input_tokens"], 18);
}

#[tokio::test]
async fn invalid_and_nested_compression_encodings_are_rejected() {
    let cases = [
        (
            vec![AUTH, (header::CONTENT_ENCODING.as_str(), "gzip")],
            vec![0x1f, 0x8b, 0x00],
            StatusCode::BAD_REQUEST,
        ),
        (
            vec![AUTH, (header::CONTENT_ENCODING.as_str(), "gzip")],
            {
                let mut valid = gzip(&body_bytes(&valid_request())).await;
                valid.truncate(valid.len().saturating_sub(5));
                valid
            },
            StatusCode::BAD_REQUEST,
        ),
        (
            vec![AUTH, (header::CONTENT_ENCODING.as_str(), "gzip, gzip")],
            gzip(&gzip(&body_bytes(&valid_request())).await).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            vec![
                AUTH,
                (header::CONTENT_ENCODING.as_str(), "gzip"),
                (header::CONTENT_ENCODING.as_str(), "gzip"),
            ],
            gzip(&body_bytes(&valid_request())).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            vec![AUTH, (header::CONTENT_ENCODING.as_str(), "br")],
            body_bytes(&valid_request()),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
    ];
    for (headers, body, expected) in cases {
        let refs: Vec<_> = headers
            .iter()
            .map(|(name, value)| (*name, *value))
            .collect();
        let (status, _) =
            response_json(call(router(), Method::POST, OPENROUTER, &refs, body).await).await;
        assert_eq!(status, expected, "encoding headers {refs:?}");
    }
}

#[tokio::test]
async fn decompressed_and_multipart_file_bytes_count_toward_the_limit() {
    let expansion = gzip(&exact_size_valid_body(MAX_BODY_BYTES + 1)).await;
    let (status, _) = response_json(
        call(
            router(),
            Method::POST,
            OPENROUTER,
            &[
                (AUTH.0, AUTH.1),
                (header::CONTENT_ENCODING.as_str(), "gzip"),
            ],
            expansion,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    let boundary = "flip-test-boundary";
    let json_part = body_bytes(&valid_request());
    let multipart = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"request\"\r\nContent-Type: application/json\r\n\r\n{}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"attachment\"; filename=\"ignored.bin\"\r\nContent-Type: application/octet-stream\r\n\r\ndummy\r\n--{boundary}--\r\n",
        String::from_utf8(json_part).unwrap()
    );
    let (status, value) = response_json(
        call(
            router(),
            Method::POST,
            OPENROUTER,
            &[
                AUTH,
                (
                    header::CONTENT_TYPE.as_str(),
                    "multipart/form-data; boundary=flip-test-boundary",
                ),
            ],
            multipart.into_bytes(),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["answers"]["q"]["type"], "noul");

    let mut multipart = Vec::new();
    multipart.extend_from_slice(b"--flip-test-boundary\r\nContent-Disposition: form-data; name=\"request\"\r\nContent-Type: application/json\r\n\r\n");
    multipart.extend_from_slice(&body_bytes(&valid_request()));
    multipart.extend_from_slice(b"\r\n--flip-test-boundary\r\nContent-Disposition: form-data; name=\"attachment\"; filename=\"ignored.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n");
    multipart.resize(MAX_BODY_BYTES + 1, b'x');
    multipart.extend_from_slice(b"\r\n--flip-test-boundary--\r\n");
    let (status, _) = response_json(
        call(
            router(),
            Method::POST,
            OPENROUTER,
            &[
                AUTH,
                (
                    header::CONTENT_TYPE.as_str(),
                    "multipart/form-data; boundary=flip-test-boundary",
                ),
            ],
            multipart,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn edge_model_sets_confidence_edges_and_preserves_score_legend() {
    let request = json!({
        "state":"edge-case",
        "model":"flip-edge",
        "questions":{
            "noul":{"type":"noul"},
            "single":{"type":"choice","criteria":{"only":"one option"}},
            "multiple":{"type":"choice","criteria":{"first":"A","second":"B"}},
            "score":{"type":"score","criteria":["low",{"label":"high"}]},
            "box":{"type":"box"}
        }
    });
    let (status, response) = post_json(router(), OPENROUTER, &request).await;
    assert_eq!(status, StatusCode::OK);
    let answers = &response["answers"];
    assert_eq!(answers["noul"]["noul"], 0.5);
    assert_eq!(answers["single"]["confidence"], 1.0);
    assert_eq!(answers["multiple"]["confidence"], 0.0);
    assert_eq!(answers["score"]["confidence"], 0.0);
    assert_eq!(answers["box"]["confidence"], 0.0);
    assert_eq!(
        answers["score"]["legend"],
        json!({"0":"low","1":{"label":"high"}})
    );
    assert_eq!(answers["score"]["score"], 0.5);
    assert_eq!(answers["score"]["probabilities"], json!({"0":0.5,"1":0.5}));
}

#[tokio::test]
async fn overloaded_alias_returns_the_retryable_529_message() {
    let mut request = valid_request();
    request["model"] = json!("flip-overloaded");
    let (status, response) = post_json(router(), OPENROUTER, &request).await;
    assert_eq!(status.as_u16(), 529);
    assert_eq!(
        response["error"]["message"],
        "The coin is fine. This alias exists so you can test retry logic."
    );
}

#[tokio::test]
async fn test_seed_makes_repeated_requests_and_question_order_deterministic() {
    let request = json!({
        "state":{"fixed":true},
        "model":"flip-latest",
        "questions":{
            "first":{"type":"choice","criteria":{"a":"A","b":"B","c":"C"}},
            "second":{"type":"score","criteria":["low","mid","high"]}
        }
    });
    let app = router_with_test_seed(true);
    let (status_a, first) = post_json_seeded(app.clone(), OPENROUTER, &request).await;
    let (status_b, repeated) = post_json_seeded(app.clone(), OPENROUTER, &request).await;
    assert_eq!(status_a, StatusCode::OK);
    assert_eq!(status_b, StatusCode::OK);
    assert_eq!(first, repeated);

    let reversed = json!({
        "state":{"fixed":true},
        "model":"flip-latest",
        "questions":{
            "second":{"type":"score","criteria":["low","mid","high"]},
            "first":{"type":"choice","criteria":{"a":"A","b":"B","c":"C"}}
        }
    });
    let (status_reversed, reordered) = post_json_seeded(app.clone(), OPENROUTER, &reversed).await;
    assert_eq!(status_reversed, StatusCode::OK);
    assert_eq!(first["answers"]["first"], reordered["answers"]["first"]);
    assert_eq!(first["answers"]["second"], reordered["answers"]["second"]);

    let (status_openrouter, openrouter) = post_json_seeded(app, OPENROUTER, &request).await;
    assert_eq!(status_openrouter, StatusCode::OK);
    assert_eq!(first["answers"], openrouter["answers"]);
}

#[tokio::test]
async fn non_test_seed_router_accepts_the_regular_request_shape() {
    let request = json!({
        "model":"flip-latest",
        "state":{"seed":"not-a-number","anything":true},
        "questions":{"q":{"type":"noul"}}
    });
    let (status, _) = response_json(
        call(
            router_with_test_seed(false),
            Method::POST,
            OPENROUTER,
            &[
                AUTH,
                ("x-flip-seed", "not-a-number"),
                (header::CONTENT_TYPE.as_str(), "application/json"),
            ],
            body_bytes(&request),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
