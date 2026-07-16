use northstar_cli::ansi;

pub fn print_contract(tool_name: &str, accepted_inputs: &str, produced_outputs: &str) {
    ansi::info(format!("{tool_name} version={}", env!("CARGO_PKG_VERSION")));
    ansi::info("production output: compact status lines for suite logs");
    ansi::info("debug output: add --debug or --verbose to print accepted formats, resolved paths and counts");
    ansi::info(format!("accepted input files: {accepted_inputs}"));
    ansi::info(format!("produced output files: {produced_outputs}"));
}

pub fn print_version(tool_name: &str) {
    println!("{tool_name} {}", env!("CARGO_PKG_VERSION"));
}

pub fn print_operation(tool_name: &str, command: &str, debug: bool, accepted_inputs: &str, produced_outputs: &str) {
    let mode = if debug { "debug" } else { "prod" };
    ansi::info(format!("{tool_name}: command={command} output_mode={mode}"));
    ansi::info(format!("accepted input files: {accepted_inputs}"));
    ansi::info(format!("produced output files: {produced_outputs}"));
    if debug {
        ansi::debug(format!("{tool_name}: debug diagnostics enabled"));
    }
}

pub fn print_debug_value<T: std::fmt::Display>(debug: bool, key: &str, value: T) {
    if debug {
        ansi::debug(format!("{key}={value}"));
    }
}

pub fn print_doctor_ok(tool_name: &str) {
    ansi::ok(format!("{tool_name} doctor passed"));
    ansi::info(format!("version={}", env!("CARGO_PKG_VERSION")));
}
