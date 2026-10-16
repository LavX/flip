use axum::http::StatusCode;
use serde_json::{Map, Value, value::RawValue};

use crate::{
    error::ApiError,
    models::{DecisionRequest, Question},
};

const MAX_NESTING_DEPTH: usize = 64;

/// Parse and validate the request body before any decision logic sees it.
pub fn parse_request(bytes: &[u8]) -> Result<DecisionRequest, ApiError> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_utf8",
            "request body is not valid UTF-8",
        )
    })?;

    if text.starts_with('\u{feff}') {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "byte_order_mark",
            "request body must not begin with a byte order mark",
        ));
    }

    let value = match serde_json::from_str::<Value>(text) {
        Ok(value) => value,
        Err(error) if error.to_string().contains("recursion limit exceeded") => {
            // RawValue validates JSON without recursively building a Value tree.
            // Only the ordinary Value parser's depth limit sends us here.
            serde_json::from_str::<&RawValue>(text).map_err(malformed_json)?;
            parse_deep_value(text.as_bytes())
        }
        Err(error) => return Err(malformed_json(error)),
    };
    let value = ValueGuard(value);

    if contains_non_finite_number(&value.0) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "non_finite_number",
            "request body contains a non-finite number",
        ));
    }

    if request_slots_exceed_depth(&value.0) {
        return Err(nesting_error());
    }

    validate_schema(&value.0)
}

fn malformed_json(error: serde_json::Error) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "malformed_json",
        format!("request body is not valid JSON: {error}"),
    )
}

/// Deep unknown fields are legal. Drain children before dropping their parent
/// so even a deeply nested ignored value cannot recurse through Value::drop.
struct ValueGuard(Value);

impl Drop for ValueGuard {
    fn drop(&mut self) {
        drop_value_iteratively(std::mem::take(&mut self.0));
    }
}

fn drop_value_iteratively(value: Value) {
    enum Children {
        Array(std::vec::IntoIter<Value>),
        Object(serde_json::map::IntoIter),
    }

    let mut pending = vec![Children::Array(vec![value].into_iter())];
    while let Some(children) = pending.last_mut() {
        let next = match children {
            Children::Array(values) => values.next(),
            Children::Object(values) => values.next().map(|(_, value)| value),
        };
        match next {
            Some(Value::Array(values)) => pending.push(Children::Array(values.into_iter())),
            Some(Value::Object(values)) => pending.push(Children::Object(values.into_iter())),
            Some(_) => {}
            None => {
                pending.pop();
            }
        }
    }
}

enum Frame {
    Array {
        values: Vec<Value>,
        wants_value: bool,
    },
    Object {
        values: Map<String, Value>,
        key: Option<String>,
        wants_key: bool,
    },
}

/// Construct a validated JSON document without recursive descent. The
/// RawValue pass has already checked syntax, including escaped strings.
fn parse_deep_value(bytes: &[u8]) -> Value {
    let mut position = 0;
    let mut frames = Vec::new();
    let mut completed = None;
    read_node(bytes, &mut position, &mut frames, &mut completed);

    loop {
        if let Some(value) = completed.take() {
            match frames.last_mut() {
                Some(Frame::Array {
                    values,
                    wants_value,
                }) => {
                    values.push(value);
                    *wants_value = false;
                }
                Some(Frame::Object { values, key, .. }) => {
                    if let Some(replaced) =
                        values.insert(key.take().expect("validated object key"), value)
                    {
                        drop_value_iteratively(replaced);
                    }
                }
                None => return value,
            }
            continue;
        }

        skip_whitespace(bytes, &mut position);
        match frames.last_mut().expect("an incomplete container") {
            Frame::Array { wants_value, .. } if *wants_value && bytes[position] != b']' => {
                read_node(bytes, &mut position, &mut frames, &mut completed);
            }
            Frame::Array { wants_value, .. } if !*wants_value && bytes[position] == b',' => {
                position += 1;
                *wants_value = true;
            }
            Frame::Array { .. } => {
                position += 1;
                let Frame::Array { values, .. } = frames.pop().unwrap() else {
                    unreachable!()
                };
                completed = Some(Value::Array(values));
            }
            Frame::Object { key, wants_key, .. } if *wants_key && bytes[position] != b'}' => {
                let end = scan_string(bytes, position);
                *key = Some(serde_json::from_slice(&bytes[position..end]).expect("validated key"));
                position = end;
                skip_whitespace(bytes, &mut position);
                position += 1; // colon
                *wants_key = false;
            }
            Frame::Object { key: Some(_), .. } => {
                read_node(bytes, &mut position, &mut frames, &mut completed);
            }
            Frame::Object { wants_key, .. } if !*wants_key && bytes[position] == b',' => {
                position += 1;
                *wants_key = true;
            }
            Frame::Object { .. } => {
                position += 1;
                let Frame::Object { values, .. } = frames.pop().unwrap() else {
                    unreachable!()
                };
                completed = Some(Value::Object(values));
            }
        }
    }
}

fn read_node(
    bytes: &[u8],
    position: &mut usize,
    frames: &mut Vec<Frame>,
    completed: &mut Option<Value>,
) {
    skip_whitespace(bytes, position);
    match bytes[*position] {
        b'[' => {
            *position += 1;
            frames.push(Frame::Array {
                values: Vec::new(),
                wants_value: true,
            });
        }
        b'{' => {
            *position += 1;
            frames.push(Frame::Object {
                values: Map::new(),
                key: None,
                wants_key: true,
            });
        }
        b'"' => {
            let end = scan_string(bytes, *position);
            *completed =
                Some(serde_json::from_slice(&bytes[*position..end]).expect("validated string"));
            *position = end;
        }
        _ => {
            let start = *position;
            while *position < bytes.len()
                && !matches!(
                    bytes[*position],
                    b' ' | b'\n' | b'\r' | b'\t' | b',' | b']' | b'}'
                )
            {
                *position += 1;
            }
            *completed =
                Some(serde_json::from_slice(&bytes[start..*position]).expect("validated scalar"));
        }
    }
}

fn scan_string(bytes: &[u8], start: usize) -> usize {
    let mut position = start + 1;
    while position < bytes.len() {
        match bytes[position] {
            b'\\' => position += 2,
            b'"' => return position + 1,
            _ => position += 1,
        }
    }
    unreachable!("RawValue validated this string")
}

fn skip_whitespace(bytes: &[u8], position: &mut usize) {
    while *position < bytes.len() && matches!(bytes[*position], b' ' | b'\n' | b'\r' | b'\t') {
        *position += 1;
    }
}

fn nesting_error() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "nesting_too_deep",
        format!("request body nesting exceeds {MAX_NESTING_DEPTH} levels"),
    )
}

fn contains_non_finite_number(root: &Value) -> bool {
    let mut pending = vec![root];

    while let Some(value) = pending.pop() {
        match value {
            Value::Number(number) => {
                if number.as_f64().is_none_or(|number| !number.is_finite()) {
                    return true;
                }
            }
            Value::Array(values) => pending.extend(values.iter()),
            Value::Object(values) => pending.extend(values.values()),
            Value::Null | Value::Bool(_) | Value::String(_) => {}
        }
    }

    false
}

/// The request envelope is shallow, so only state and each question's
/// instructions and criteria contribute to the documented nesting limit.
fn request_slots_exceed_depth(root: &Value) -> bool {
    let Some(object) = root.as_object() else {
        return false;
    };

    if object.get("state").is_some_and(exceeds_depth_limit) {
        return true;
    }

    let Some(questions) = object.get("questions").and_then(Value::as_object) else {
        return false;
    };

    questions.values().any(|question| {
        let Some(question) = question.as_object() else {
            return false;
        };

        ["instructions", "criteria"]
            .into_iter()
            .filter_map(|field| question.get(field))
            .any(exceeds_depth_limit)
    })
}

fn exceeds_depth_limit(root: &Value) -> bool {
    let mut pending = vec![(root, 0_usize)];

    while let Some((value, parent_depth)) = pending.pop() {
        match value {
            Value::Array(values) => {
                let depth = parent_depth + 1;
                if depth > MAX_NESTING_DEPTH {
                    return true;
                }
                pending.extend(values.iter().map(|value| (value, depth)));
            }
            Value::Object(values) => {
                let depth = parent_depth + 1;
                if depth > MAX_NESTING_DEPTH {
                    return true;
                }
                pending.extend(values.values().map(|value| (value, depth)));
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    false
}

fn validate_schema(root: &Value) -> Result<DecisionRequest, ApiError> {
    let object = root
        .as_object()
        .ok_or_else(|| ApiError::invalid("request", "must be an object"))?;

    let model = object
        .get("model")
        .ok_or_else(|| ApiError::invalid("model", "is required"))?
        .as_str()
        .ok_or_else(|| ApiError::invalid("model", "must be a string"))?;
    if model.is_empty() {
        return Err(ApiError::invalid("model", "must be a non-empty string"));
    }

    let state = object
        .get("state")
        .ok_or_else(|| ApiError::invalid("state", "is required"))?;
    if !(state.is_string() || state.is_object() || state.is_array()) {
        return Err(ApiError::invalid(
            "state",
            "must be a string, object, or array",
        ));
    }

    let questions = object
        .get("questions")
        .ok_or_else(|| ApiError::invalid("questions", "is required"))?
        .as_object()
        .ok_or_else(|| ApiError::invalid("questions", "must be an object"))?;
    if questions.is_empty() {
        return Err(ApiError::invalid(
            "questions",
            "must contain at least one question",
        ));
    }

    let mut validated = Vec::with_capacity(questions.len());
    for (name, value) in questions {
        let path = format!("questions.{name}");
        let question = value
            .as_object()
            .ok_or_else(|| ApiError::invalid(&path, "must be an object"))?;
        let type_path = format!("{path}.type");
        let question_type = question
            .get("type")
            .ok_or_else(|| ApiError::invalid(&type_path, "is required"))?
            .as_str()
            .ok_or_else(|| ApiError::invalid(&type_path, "must be a string"))?;

        // SDK compatibility permits missing or null instructions, plus text or structured content.
        if let Some(instructions) = question.get("instructions") {
            let instructions_path = format!("{path}.instructions");
            if !is_flexible_value(instructions, true) {
                return Err(ApiError::invalid(
                    &instructions_path,
                    "must be a string, object, array, or null",
                ));
            }
        }

        let validated_question = match question_type {
            "noul" => {
                validate_noul(question.get("criteria"), &path)?;
                Question::Noul
            }
            "choice" => {
                let criteria_path = format!("{path}.criteria");
                let criteria = question
                    .get("criteria")
                    .ok_or_else(|| ApiError::invalid(&criteria_path, "is required"))?
                    .as_object()
                    .ok_or_else(|| ApiError::invalid(&criteria_path, "must be an object"))?;
                if criteria.is_empty() {
                    return Err(ApiError::invalid(
                        &criteria_path,
                        "must contain at least one choice",
                    ));
                }
                if criteria.len() > 255 {
                    return Err(ApiError::invalid(
                        &criteria_path,
                        "must contain at most 255 choices",
                    ));
                }
                for (choice, value) in criteria {
                    let value_path = format!("{criteria_path}.{choice}");
                    if !is_flexible_value(value, true) {
                        return Err(ApiError::invalid(
                            &value_path,
                            "must be a string, object, array, or null",
                        ));
                    }
                }
                Question::Choice(criteria.clone())
            }
            "score" => {
                let criteria_path = format!("{path}.criteria");
                let criteria = question
                    .get("criteria")
                    .ok_or_else(|| ApiError::invalid(&criteria_path, "is required"))?
                    .as_array()
                    .ok_or_else(|| ApiError::invalid(&criteria_path, "must be an array"))?;
                if !(2..=10).contains(&criteria.len()) {
                    return Err(ApiError::invalid(
                        &criteria_path,
                        "must contain between 2 and 10 entries",
                    ));
                }
                for (index, value) in criteria.iter().enumerate() {
                    if !is_flexible_value(value, true) {
                        let value_path = format!("{criteria_path}[{index}]");
                        return Err(ApiError::invalid(
                            &value_path,
                            "must be a string, object, array, or null",
                        ));
                    }
                }
                Question::Score(criteria.clone())
            }
            "box" => Question::Box,
            _ => {
                return Err(ApiError::invalid(
                    &type_path,
                    "must be one of: noul, choice, score, box",
                ));
            }
        };

        validated.push((name.clone(), validated_question));
    }

    Ok(DecisionRequest {
        model: model.to_owned(),
        questions: validated,
    })
}

fn validate_noul(criteria: Option<&Value>, question_path: &str) -> Result<(), ApiError> {
    let Some(criteria) = criteria else {
        return Ok(());
    };

    let criteria_path = format!("{question_path}.criteria");
    let criteria = criteria
        .as_object()
        .ok_or_else(|| ApiError::invalid(&criteria_path, "must be an object"))?;

    for answer in ["true", "false"] {
        if let Some(value) = criteria.get(answer)
            && !is_flexible_value(value, true)
        {
            let value_path = format!("{criteria_path}.{answer}");
            return Err(ApiError::invalid(
                &value_path,
                "must be a string, object, array, or null",
            ));
        }
    }

    Ok(())
}

fn is_flexible_value(value: &Value, allow_null: bool) -> bool {
    (allow_null && value.is_null()) || value.is_string() || value.is_object() || value.is_array()
}

#[cfg(test)]
mod tests {
    use super::parse_request;
    use crate::models::Question;
    use axum::http::StatusCode;

    fn parse(input: &str) -> Result<crate::models::DecisionRequest, crate::error::ApiError> {
        parse_request(input.as_bytes())
    }

    fn question_body(question: &str) -> String {
        format!(
            r#"{{"model":"alias/from-provider","state":{{"ok":true}},"questions":{{"next_step":{question}}}}}"#
        )
    }

    fn nested_arrays(depth: usize) -> String {
        format!("{}\"leaf\"{}", "[".repeat(depth), "]".repeat(depth))
    }

    fn assert_error(input: &[u8], status: StatusCode, code: &str, message_part: &str) {
        let err = parse_request(input).expect_err("request should be rejected");
        assert_eq!(err.status, status);
        assert_eq!(err.code, code);
        assert!(err.message.contains(message_part), "{}", err.message);
    }

    #[test]
    fn accepts_alias_models_lenient_optional_fields_and_unknown_fields() {
        let req = parse(
            r#"{
                "model":"openrouter/vendor-model",
                "state":["preserved", 1],
                "questions":{
                    "continue":{"type":"noul","instructions":null,"criteria":{"true":null,"extra":false}},
                    "choice":{"type":"choice","criteria":{"first":null,"second":{"label":"Second"}}},
                    "score":{"type":"score","criteria":[null,{"label":"Great"}]},
                    "box":{"type":"box","criteria":17,"ignored":true}
                },
                "future_field":{"ignored":true}
            }"#,
        )
        .expect("valid request");

        assert_eq!(req.model, "openrouter/vendor-model");
        assert_eq!(req.questions.len(), 4);
        assert!(matches!(req.questions[0].1, Question::Noul));
        match &req.questions[1].1 {
            Question::Choice(criteria) => {
                assert!(criteria["first"].is_null());
                assert_eq!(criteria["second"]["label"], "Second");
            }
            other => panic!("expected choice, got {other:?}"),
        }
        match &req.questions[2].1 {
            Question::Score(criteria) => {
                assert!(criteria[0].is_null());
                assert_eq!(criteria[1]["label"], "Great");
            }
            other => panic!("expected score, got {other:?}"),
        }
        assert!(matches!(req.questions[3].1, Question::Box));
    }

    #[test]
    fn accepts_missing_noul_criteria_and_missing_instructions() {
        let req = parse(&question_body(r#"{"type":"noul"}"#)).expect("valid request");
        assert!(matches!(req.questions[0].1, Question::Noul));
    }

    #[test]
    fn rejects_bad_schema_with_the_exact_field_path() {
        let input = question_body(r#"{"type":"choice"}"#);
        let err = parse(&input).expect_err("choice criteria is required");
        assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.code, "invalid_request");
        assert_eq!(err.message, "questions.next_step.criteria: is required");
    }

    #[test]
    fn enforces_choice_and_score_criteria_limits() {
        let choices = |count: usize| {
            let values = (0..count)
                .map(|index| format!("\"choice_{index}\":null"))
                .collect::<Vec<_>>()
                .join(",");
            question_body(&format!(r#"{{"type":"choice","criteria":{{{values}}}}}"#))
        };
        assert!(parse(&choices(255)).is_ok());
        let too_many_choices = choices(256);
        let err = parse(&too_many_choices).expect_err("256 choices exceed the limit");
        assert_eq!(
            err.message,
            "questions.next_step.criteria: must contain at most 255 choices"
        );

        let scores = |count: usize| {
            let values = std::iter::repeat_n("null", count)
                .collect::<Vec<_>>()
                .join(",");
            question_body(&format!(r#"{{"type":"score","criteria":[{values}]}}"#))
        };
        assert!(parse(&scores(2)).is_ok());
        assert!(parse(&scores(10)).is_ok());
        for count in [1, 11] {
            let err = parse(&scores(count)).expect_err("score criteria is outside 2 to 10");
            assert_eq!(
                err.message,
                "questions.next_step.criteria: must contain between 2 and 10 entries"
            );
        }
    }

    #[test]
    fn reports_nested_schema_paths() {
        let input = question_body(r#"{"type":"noul","criteria":{"true":false}}"#);
        let err = parse(&input).expect_err("boolean criterion is not accepted");
        assert_eq!(
            err.message,
            "questions.next_step.criteria.true: must be a string, object, array, or null"
        );

        let input = question_body(r#"{"type":"box","instructions":false}"#);
        let err = parse(&input).expect_err("boolean instructions are not accepted");
        assert_eq!(
            err.message,
            "questions.next_step.instructions: must be a string, object, array, or null"
        );
    }

    #[test]
    fn rejects_invalid_utf8_before_json_parsing() {
        assert_error(
            b"{\"model\":\"m\",\"state\":\"\xff\",\"questions\":{}}",
            StatusCode::BAD_REQUEST,
            "invalid_utf8",
            "UTF-8",
        );
    }

    #[test]
    fn rejects_a_leading_utf8_bom() {
        assert_error(
            b"\xef\xbb\xbf{}",
            StatusCode::BAD_REQUEST,
            "byte_order_mark",
            "byte order mark",
        );
    }

    #[test]
    fn rejects_malformed_json() {
        assert_error(
            b"{\"model\":",
            StatusCode::BAD_REQUEST,
            "malformed_json",
            "JSON",
        );
    }

    #[test]
    fn rejects_numbers_that_cannot_be_finite_f64_values() {
        assert_error(
            br#"{"model":"m","state":1e400,"questions":{"q":{"type":"box"}}}"#,
            StatusCode::BAD_REQUEST,
            "non_finite_number",
            "non-finite",
        );
    }

    #[test]
    fn checks_depth_limit_in_state_instructions_and_criteria() {
        let state_64 = format!(
            r#"{{"model":"m","state":{},"questions":{{"q":{{"type":"box"}}}}}}"#,
            nested_arrays(64)
        );
        assert!(parse(&state_64).is_ok());

        let state_65 = state_64.replace(&nested_arrays(64), &nested_arrays(65));
        assert_error(
            state_65.as_bytes(),
            StatusCode::BAD_REQUEST,
            "nesting_too_deep",
            "64",
        );

        let state_129 = state_64.replace(&nested_arrays(64), &nested_arrays(129));
        assert_error(
            state_129.as_bytes(),
            StatusCode::BAD_REQUEST,
            "nesting_too_deep",
            "64",
        );

        let instructions_64 = question_body(&format!(
            r#"{{"type":"noul","instructions":{}}}"#,
            nested_arrays(64)
        ));
        assert!(parse(&instructions_64).is_ok());

        let instructions_65 = question_body(&format!(
            r#"{{"type":"noul","instructions":{}}}"#,
            nested_arrays(65)
        ));
        assert_error(
            instructions_65.as_bytes(),
            StatusCode::BAD_REQUEST,
            "nesting_too_deep",
            "64",
        );

        let criteria_64 = question_body(&format!(
            r#"{{"type":"noul","criteria":{{"true":{}}}}}"#,
            nested_arrays(63)
        ));
        assert!(parse(&criteria_64).is_ok());

        let criteria_65 = question_body(&format!(
            r#"{{"type":"noul","criteria":{{"true":{}}}}}"#,
            nested_arrays(64)
        ));
        assert_error(
            criteria_65.as_bytes(),
            StatusCode::BAD_REQUEST,
            "nesting_too_deep",
            "64",
        );
    }

    #[test]
    fn accepts_deep_unknown_fields() {
        for depth in [127, 10_000] {
            let unknown = nested_arrays(depth);
            let top_level = format!(
                r#"{{"model":"m","state":"s","questions":{{"q":{{"type":"box"}}}},"future":{unknown}}}"#
            );
            assert!(
                parse(&top_level).is_ok(),
                "top-level unknown field at depth {depth}"
            );

            let question = question_body(&format!(r#"{{"type":"box","future":{unknown}}}"#));
            assert!(
                parse(&question).is_ok(),
                "question unknown field at depth {depth}"
            );
        }

        let nested_objects = format!(
            "{}{{\"escaped\\\"key\":[true,null,1.5]}}{}",
            "{\"child\":".repeat(10_000),
            "}".repeat(10_000)
        );
        let body = format!(
            r#"{{"model":"m","state":"s","questions":{{"q":{{"type":"box"}}}},"future":{nested_objects}}}"#
        );
        assert!(
            parse(&body).is_ok(),
            "nested unknown objects with an escaped key"
        );
    }

    #[test]
    fn validates_numbers_and_syntax_inside_deep_unknown_fields() {
        let unknown = format!("{}1e400{}", "[".repeat(1_000), "]".repeat(1_000));
        let non_finite = format!(
            r#"{{"model":"m","state":"s","questions":{{"q":{{"type":"box"}}}},"future":{unknown}}}"#
        );
        assert_error(
            non_finite.as_bytes(),
            StatusCode::BAD_REQUEST,
            "non_finite_number",
            "non-finite",
        );

        let malformed = format!(
            r#"{{"model":"m","state":"s","questions":{{"q":{{"type":"box"}}}},"future":{}0{}}}"#,
            "[".repeat(1_000),
            "]".repeat(999)
        );
        assert_error(
            malformed.as_bytes(),
            StatusCode::BAD_REQUEST,
            "malformed_json",
            "JSON",
        );
    }

    #[test]
    fn accepts_deep_duplicate_field_without_recursive_drop() {
        let unknown = nested_arrays(10_000);
        let body = format!(
            r#"{{"model":"m","state":"s","questions":{{"q":{{"type":"box"}}}},"future":{unknown},"future":0}}"#
        );
        assert!(parse(&body).is_ok());
    }
}
