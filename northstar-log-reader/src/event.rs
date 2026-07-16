#![forbid(unsafe_op_in_unsafe_fn)]

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct NormalizedLogEvent {
    pub schema: String,
    pub sequence: Option<u64>,
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub event_id: String,
    pub message: String,
    pub fields: Value,
    pub raw: Value,
}

pub fn parse_live_payload(payload: &str) -> Result<Option<NormalizedLogEvent>, String> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("parse live event failed: {e}; payload={trimmed}"))?;
    Ok(normalize_event(value))
}

pub fn parse_events(body: &str) -> Result<Vec<NormalizedLogEvent>, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if trimmed.starts_with('[') {
        let values: Vec<Value> = serde_json::from_str(trimmed)
            .map_err(|e| format!("parse JSON array failed: {e}"))?;
        return Ok(values.into_iter().filter_map(normalize_event).collect());
    }

    if trimmed.starts_with('{') && !trimmed.contains('\n') {
        let value: Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("parse JSON object failed: {e}"))?;
        if let Some(events) = value.get("events").and_then(Value::as_array) {
            return Ok(events.iter().cloned().filter_map(normalize_event).collect());
        }
        return Ok(normalize_event(value).into_iter().collect());
    }

    let mut events = Vec::new();
    for (idx, line) in body.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let event_line = line.strip_prefix("data:").map(str::trim_start).unwrap_or(line);
        match serde_json::from_str::<Value>(event_line) {
            Ok(value) => {
                if let Some(event) = normalize_event(value) {
                    events.push(event);
                }
            }
            Err(e) => return Err(format!("parse JSONL failed at line {}: {e}", idx + 1)),
        }
    }
    Ok(events)
}

fn normalize_event(value: Value) -> Option<NormalizedLogEvent> {
    let obj = value.as_object()?;
    let schema = obj
        .get("schema")
        .and_then(Value::as_str)
        .unwrap_or("northstar.logging.unknown.v1")
        .to_owned();

    if schema == "northstar.ulog.event.v1" {
        let source = obj.get("source").and_then(Value::as_object);
        return Some(NormalizedLogEvent {
            schema,
            sequence: None,
            timestamp: obj.get("timestamp_utc").and_then(Value::as_str).unwrap_or("-").to_owned(),
            level: obj.get("level").and_then(Value::as_str).unwrap_or("INFO").to_owned(),
            target: source
                .and_then(|s| s.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("-")
                .to_owned(),
            event_id: obj.get("event_id").and_then(Value::as_str).unwrap_or("-").to_owned(),
            message: obj.get("message").and_then(Value::as_str).unwrap_or("").to_owned(),
            fields: obj.get("fields").cloned().unwrap_or(Value::Null),
            raw: value,
        });
    }

    Some(NormalizedLogEvent {
        schema,
        sequence: obj.get("sequence").and_then(Value::as_u64),
        timestamp: obj
            .get("timestamp_utc")
            .or_else(|| obj.get("timestamp"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| obj.get("timestamp_ns").and_then(Value::as_u64).map(|v| format!("{v}ns")))
            .unwrap_or_else(|| "-".to_owned()),
        level: obj.get("level").and_then(Value::as_str).unwrap_or("INFO").to_owned(),
        target: obj
            .get("target")
            .or_else(|| obj.get("source"))
            .and_then(Value::as_str)
            .unwrap_or("-")
            .to_owned(),
        event_id: obj.get("event_id").and_then(Value::as_str).unwrap_or("-").to_owned(),
        message: obj.get("message").and_then(Value::as_str).unwrap_or("").to_owned(),
        fields: obj.get("fields").cloned().unwrap_or(Value::Null),
        raw: value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_logging_event_jsonl() {
        let body = r#"{"schema":"northstar.logging.event.v1","sequence":1,"timestamp_ns":10,"level":"WARN","target":"engine.logging","message":"hello"}"#;
        let events = parse_events(body).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target, "engine.logging");
        assert_eq!(events[0].level, "WARN");
    }

    #[test]
    fn parses_sse_data_line() {
        let event = parse_live_payload(r#"{"schema":"northstar.logging.event.v1","sequence":1,"level":"INFO","target":"engine","message":"live"}"#)
            .unwrap()
            .unwrap();
        assert_eq!(event.message, "live");
    }
}
