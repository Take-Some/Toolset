#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{event::{parse_events, NormalizedLogEvent}, http};
use std::fs;

pub fn load_events_from_input(input: &str) -> Result<Vec<NormalizedLogEvent>, String> {
    let body = read_input_to_string(input)?;
    parse_events(&body)
}

fn read_input_to_string(input: &str) -> Result<String, String> {
    if let Some(rest) = input.strip_prefix("file://") {
        return fs::read_to_string(rest).map_err(|e| format!("read file URL '{input}' failed: {e}"));
    }
    if input.starts_with("http://") {
        return http::http_get_snapshot_to_string(input, http::DEFAULT_TIMEOUT_MS, http::DEFAULT_MAX_BYTES);
    }
    if input.starts_with("https://") {
        return Err("https URL is browser UI only in this slice; native CLI reader uses local http://".to_owned());
    }
    fs::read_to_string(input).map_err(|e| format!("read '{input}' failed: {e}"))
}
