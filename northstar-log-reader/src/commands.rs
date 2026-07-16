#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{app, http, input, output, ui};
use northstar_cli::ansi;
use std::{env, fs, path::{Path, PathBuf}, process::Command};

const DEFAULT_LOG_URL: &str = "http://127.0.0.1:8765/logs";
const VALUE_FLAGS: &[&str] = &[
    "--url",
    "--format",
    "--limit",
    "--count",
    "--max-events",
    "--out",
];

pub fn cmd_read(args: &[String]) -> Result<(), String> {
    let input = input_or_default(args, "--url");
    let format = arg_value(args, "--format").unwrap_or_else(|| "table".to_owned());
    let limit = arg_value(args, "--limit").and_then(|v| v.parse::<usize>().ok());
    let events = input::load_events_from_input(&input)?;
    let events = apply_limit(events, limit);
    output::write_events(&events, &format)
}

pub fn cmd_tail(args: &[String]) -> Result<(), String> {
    let input = input_or_default(args, "--url");
    let format = arg_value(args, "--format").unwrap_or_else(|| "table".to_owned());
    let count = arg_value(args, "--count").and_then(|v| v.parse::<usize>().ok()).unwrap_or(50);
    let events = input::load_events_from_input(&input)?;
    let start = events.len().saturating_sub(count);
    output::write_events(&events[start..], &format)
}

pub fn cmd_live(args: &[String]) -> Result<(), String> {
    let input = input_or_default(args, "--url");
    let format = arg_value(args, "--format").unwrap_or_else(|| "table".to_owned());
    let max_events = arg_value(args, "--max-events").and_then(|v| v.parse::<usize>().ok());

    if !input.starts_with("http://") {
        return Err("live requires an http:// streaming endpoint; use read/tail for files or snapshots".to_owned());
    }

    http::stream_http_events(&input, max_events, |event| output::write_event(event, &format))
}

pub fn cmd_ui(args: &[String]) -> Result<(), String> {
    let input = input_or_default(args, "--url");
    if !input.starts_with("http://") {
        return Err("native app currently supports http:// live URLs; leave --url empty and edit inside UI or pass http://...".to_owned());
    }
    app::run(&input)
}

pub fn cmd_html(args: &[String]) -> Result<(), String> {
    let input = input_or_default(args, "--url");
    if !input.starts_with("http://") && !input.starts_with("https://") {
        return Err("--url must be an HTTP(S) streaming URL when provided".to_owned());
    }

    let path = arg_value(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(default_ui_path);

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("create ui output dir '{}' failed: {e}", parent.display()))?;
        }
    }

    let html = ui::render_live_html(&input);
    fs::write(&path, html).map_err(|e| format!("write ui '{}' failed: {e}", path.display()))?;

    ansi::ok(format!("wrote LIVE LogReader HTML UI: {}", path.display()));
    ansi::info(format!("source_url={input}"));

    if !has_flag(args, "--no-open") {
        open_ui_file(&path)?;
        ansi::ok("opened LIVE LogReader HTML UI");
    }

    Ok(())
}

fn default_ui_path() -> PathBuf {
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("northstar-log-reader-ui.html");
        }
    }
    PathBuf::from("northstar-log-reader-ui.html")
}

fn open_ui_file(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &path.display().to_string()])
            .spawn()
            .map_err(|e| format!("open ui '{}' failed: {e}", path.display()))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("open ui '{}' failed: {e}", path.display()))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("open ui '{}' failed: {e}", path.display()))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(format!("open ui is not supported on this platform; file={}", path.display()))
}

fn input_or_default(args: &[String], name: &str) -> String {
    arg_value(args, name)
        .or_else(|| positional_input(args))
        .unwrap_or_else(|| DEFAULT_LOG_URL.to_owned())
}

fn positional_input(args: &[String]) -> Option<String> {
    args.iter().enumerate().find_map(|(index, arg)| {
        if arg.starts_with('-') {
            return None;
        }

        if index > 0 && VALUE_FLAGS.contains(&args[index - 1].as_str()) {
            return None;
        }

        Some(arg.clone())
    })
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|pair| pair[0] == name).map(|pair| pair[1].clone())
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn apply_limit(mut events: Vec<crate::event::NormalizedLogEvent>, limit: Option<usize>) -> Vec<crate::event::NormalizedLogEvent> {
    if let Some(limit) = limit {
        events.truncate(limit);
    }
    events
}
