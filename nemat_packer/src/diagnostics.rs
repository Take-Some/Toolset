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

pub fn print_doctor_ok(tool_name: &str) {
    ansi::ok(format!("{tool_name} doctor passed"));
    ansi::info(format!("version={}", env!("CARGO_PKG_VERSION")));
}
