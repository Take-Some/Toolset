#![forbid(unsafe_op_in_unsafe_fn)]

use crate::event::{parse_live_payload, NormalizedLogEvent};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    time::Duration,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_TIMEOUT_MS: u64 = 2_000;
pub const DEFAULT_MAX_BYTES: usize = 8 * 1024 * 1024;

pub fn http_get_snapshot_to_string(url: &str, timeout_ms: u64, max_bytes: usize) -> Result<String, String> {
    let parsed = HttpUrl::parse(url)?;
    let mut stream = open_http_stream(&parsed, Some(Duration::from_millis(timeout_ms.max(100))))?;
    let mut bytes = Vec::new();
    let mut buf = [0u8; 16 * 1024];

    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                bytes.extend_from_slice(&buf[..n]);
                if bytes.len() > max_bytes {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(format!("read response failed: {e}")),
        }
    }

    let response = String::from_utf8_lossy(&bytes).to_string();
    split_http_body(&response)
}

pub fn stream_http_events<F>(url: &str, max_events: Option<usize>, mut on_event: F) -> Result<(), String>
where
    F: FnMut(&NormalizedLogEvent) -> Result<(), String>,
{
    let parsed = HttpUrl::parse(url)?;
    let stream = open_http_stream(&parsed, None)?;
    stream
        .set_read_timeout(None)
        .map_err(|e| format!("clear read timeout failed: {e}"))?;

    let mut reader = BufReader::new(stream);
    read_http_headers(&mut reader)?;

    let mut count = 0usize;
    let mut sse_data = String::new();
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("read live line failed: {e}"))?;
        if n == 0 {
            break;
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            if !sse_data.trim().is_empty() {
                if emit_live_payload(&sse_data, &mut on_event, &mut count, max_events)? {
                    break;
                }
                sse_data.clear();
            }
            continue;
        }

        if let Some(data) = trimmed.strip_prefix("data:") {
            if !sse_data.is_empty() {
                sse_data.push('\n');
            }
            sse_data.push_str(data.trim_start());
            continue;
        }

        if trimmed.starts_with(':')
            || trimmed.starts_with("event:")
            || trimmed.starts_with("id:")
            || trimmed.starts_with("retry:")
        {
            continue;
        }

        if emit_live_payload(trimmed, &mut on_event, &mut count, max_events)? {
            break;
        }
    }

    Ok(())
}

fn emit_live_payload<F>(
    payload: &str,
    on_event: &mut F,
    count: &mut usize,
    max_events: Option<usize>,
) -> Result<bool, String>
where
    F: FnMut(&NormalizedLogEvent) -> Result<(), String>,
{
    if let Some(event) = parse_live_payload(payload)? {
        on_event(&event)?;
        *count = count.saturating_add(1);
        if max_events.is_some_and(|max| *count >= max) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn open_http_stream(parsed: &HttpUrl, timeout: Option<Duration>) -> Result<TcpStream, String> {
    let mut stream = TcpStream::connect((parsed.host.as_str(), parsed.port))
        .map_err(|e| format!("connect 'http://{}{}' failed: {e}", parsed.host_header, parsed.path_and_query))?;
    stream
        .set_read_timeout(timeout)
        .map_err(|e| format!("set read timeout failed: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_millis(DEFAULT_TIMEOUT_MS)))
        .map_err(|e| format!("set write timeout failed: {e}"))?;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: northstar-log-reader/{VERSION}\r\nAccept: application/x-ndjson, text/event-stream, application/json, text/plain, */*\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        parsed.path_and_query, parsed.host_header
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write request failed: {e}"))?;
    Ok(stream)
}

fn read_http_headers(reader: &mut BufReader<TcpStream>) -> Result<(), String> {
    let mut status = String::new();
    reader
        .read_line(&mut status)
        .map_err(|e| format!("read HTTP status failed: {e}"))?;
    if !(status.contains(" 200 ") || status.contains(" 206 ")) {
        return Err(format!("HTTP request failed: {}", status.trim()));
    }

    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("read HTTP header failed: {e}"))?;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }
    Ok(())
}

fn split_http_body(response: &str) -> Result<String, String> {
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "invalid HTTP response".to_owned())?;
    let status = head.lines().next().unwrap_or_default();
    if !(status.contains(" 200 ") || status.contains(" 206 ")) {
        return Err(format!("HTTP request failed: {status}"));
    }
    Ok(body.to_owned())
}

#[derive(Debug)]
struct HttpUrl {
    host: String,
    host_header: String,
    port: u16,
    path_and_query: String,
}

impl HttpUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| "only http:// URLs are supported by native CLI live reader".to_owned())?;
        let (authority, path) = match rest.split_once('/') {
            Some((authority, path)) => (authority, format!("/{path}")),
            None => (rest, "/".to_owned()),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port_text)) if !host.is_empty() => {
                let port = port_text
                    .parse::<u16>()
                    .map_err(|_| format!("invalid URL port: {port_text}"))?;
                (host.to_owned(), port)
            }
            _ => (authority.to_owned(), 80),
        };
        if host.trim().is_empty() {
            return Err("URL host is empty".to_owned());
        }
        let host_header = if port == 80 { host.clone() } else { format!("{host}:{port}") };
        Ok(Self { host, host_header, port, path_and_query: path })
    }
}
