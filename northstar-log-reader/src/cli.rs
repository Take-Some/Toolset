#![forbid(unsafe_op_in_unsafe_fn)]

use crate::commands;
use northstar_cli::ansi;

pub const TOOL_NAME: &str = "northstar-log-reader";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ACCEPTED_INPUTS: &str = "default http://127.0.0.1:8765/logs endpoint, live http:// NDJSON/SSE stream, file:// URL, local *.jsonl / *.ulog.jsonl path";
pub const PRODUCED_OUTPUTS: &str = "native LIVE window app, LIVE JSONL, table text, self-contained live *.html fallback UI";

pub fn dispatch(args: Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        return commands::cmd_ui(&[]);
    }
    if matches!(args[0].as_str(), "--help" | "-h" | "help") {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "version" | "--version" | "-V" => {
            println!("{TOOL_NAME} {VERSION}");
            Ok(())
        }
        "accepted-inputs" | "--accepted-inputs" | "inputs" | "formats" => {
            ansi::info(format!("{TOOL_NAME} version={VERSION}"));
            ansi::info(format!("accepted inputs: {ACCEPTED_INPUTS}"));
            ansi::info(format!("produced outputs: {PRODUCED_OUTPUTS}"));
            Ok(())
        }
        "doctor" | "--doctor" => {
            ansi::ok(format!("{TOOL_NAME} doctor passed"));
            ansi::info(format!("version={VERSION}"));
            ansi::info("live transport=http ndjson/sse");
            ansi::info("ui command opens native window app: northstar-log-reader ui");
            ansi::info("html command writes browser fallback: northstar-log-reader html --no-open");
            ansi::info("ui layout=log-field + bottom level/search/clear bar");
            Ok(())
        }
        "read" => commands::cmd_read(&args[1..]),
        "tail" => commands::cmd_tail(&args[1..]),
        "live" | "listen" | "stream" => commands::cmd_live(&args[1..]),
        "ui" | "app" | "window" => commands::cmd_ui(&args[1..]),
        "html" | "html-ui" | "write-ui" => commands::cmd_html(&args[1..]),
        other => Err(format!("unknown command '{other}'. Use --help to list supported commands.")),
    }
}

fn print_help() {
    println!(r#"North Star Log Reader {VERSION}

USAGE:
  northstar-log-reader                       # opens native UI
  northstar-log-reader --help
  northstar-log-reader version
  northstar-log-reader accepted-inputs
  northstar-log-reader doctor

  northstar-log-reader ui
  northstar-log-reader ui [--url <http://host/live>]   # default: http://127.0.0.1:8765/logs

  northstar-log-reader html [--url <http://host/live>] [--out log-reader.html] [--no-open]   # default URL: http://127.0.0.1:8765/logs

  northstar-log-reader read [--url <http://host/snapshot|file://path|path>] [--format table|jsonl|json] [--limit N]
  northstar-log-reader tail [--url <http://host/snapshot|file://path|path>] [--count N] [--format table|jsonl|json]
  northstar-log-reader live [--url <http://host/live>] [--format table|jsonl] [--max-events N]

UI COMMAND:
  `ui` opens the native LogReader app window.
  No arguments are required.
  Without --url, commands use http://127.0.0.1:8765/logs.
  With --url, the live URL field is prefilled with the provided value.

APP LAYOUT:
  Top:    live URL + connect/disconnect status.
  Center: full log history field.
  Bottom: [level box] [full-width log search field] [clear history logs].

LIVE CONTRACT:
  The live endpoint must keep the HTTP connection open and publish one event per line.
  Supported wire formats:
    NDJSON: {{json event}}\n
    SSE:    data: {{json event}}\n\n

SUPPORTED EVENT SHAPES:
  northstar.ulog.event.v1
  northstar.logging.event.v1
  legacy LogRecordWire-like objects with level/target/message fields

OUTPUT DISCIPLINE:
  Status commands print [INFO]/[OK] tags.
  Payload commands write clean stdout suitable for redirects.
"#);
}
