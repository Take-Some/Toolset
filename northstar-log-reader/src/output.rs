#![forbid(unsafe_op_in_unsafe_fn)]

use crate::event::NormalizedLogEvent;
use serde_json::json;

pub fn write_events(events: &[NormalizedLogEvent], format: &str) -> Result<(), String> {
    for event in events {
        write_event(event, format)?;
    }
    Ok(())
}

pub fn write_event(event: &NormalizedLogEvent, format: &str) -> Result<(), String> {
    match format.to_ascii_lowercase().as_str() {
        "jsonl" | "ndjson" => {
            println!("{}", serde_json::to_string(event).map_err(|e| e.to_string())?);
            Ok(())
        }
        "json" => {
            serde_json::to_writer_pretty(std::io::stdout(), &json!({
                "schema": "northstar.log_reader.event.v1",
                "event": event,
            }))
            .map_err(|e| format!("write json failed: {e}"))?;
            println!();
            Ok(())
        }
        "table" | "text" => {
            println!(
                "{} {:>5} {:<36} {}",
                event.timestamp,
                event.level,
                truncate(&event.target, 36),
                event.message.replace('\n', " ")
            );
            Ok(())
        }
        other => Err(format!("unsupported output format '{other}'")),
    }
}

fn truncate(value: &str, width: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= width {
        return value.to_owned();
    }
    let mut out = chars.into_iter().take(width.saturating_sub(1)).collect::<String>();
    out.push('…');
    out
}
