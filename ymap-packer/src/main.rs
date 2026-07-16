mod args;
mod commands;
mod diagnostics;
mod discovery;
mod help;
mod nef8;
mod source_doc;

fn main() {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let command = raw_args
        .first()
        .cloned()
        .unwrap_or_else(|| "help".to_owned());
    let telemetry = northstar_cli::ulog::ToolRunInstrumentation::start(
        "northstar-ymap-packer",
        command,
        &raw_args,
    );
    let args = northstar_cli::ulog::strip_ulog_args(raw_args);
    match commands::dispatch(args) {
        Ok(()) => telemetry.complete(),
        Err(err) => {
            telemetry.failed(&err);
            northstar_cli::ansi::error(format!("{err}"));
            std::process::exit(1);
        }
    }
}
