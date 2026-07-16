use crate::{cli::{TOOL_NAME, VERSION}, pe};
use northstar_cli::ansi;

pub const ACCEPTED_INPUTS: &str = "*.exe, *.dll, *.obj, *.lib with PE/COFF export or symbol-table data";
pub const RESERVED_INPUTS: &str = "*.pdb via -searchpath when DIA/PDB provider lands";
pub const PRODUCED_OUTPUTS: &str = "raw symbol list text: <symbol> 0x<RVA_HEX8> <source>";

pub fn print_version() {
    println!("{TOOL_NAME} {VERSION}");
}

pub fn print_accepted_inputs() {
    ansi::info(format!("{TOOL_NAME} version={VERSION}"));
    ansi::info(format!("accepted input files: {ACCEPTED_INPUTS}"));
    ansi::info(format!("reserved input files: {RESERVED_INPUTS}"));
    ansi::info(format!("produced output files: {PRODUCED_OUTPUTS}"));
    ansi::info("payload stdout is clean: no ANSI, no [INFO]/[OK]/[WARN]/[ERROR], no progress text");
}

pub fn doctor() -> i32 {
    let rejects_invalid = pe::parse_symbols(b"not a portable executable").is_err();
    let parses_empty_pe = pe::parse_symbols(&pe::minimal_empty_pe()).map(|v| v.is_empty()).unwrap_or(false);

    if rejects_invalid && parses_empty_pe {
        ansi::ok(format!("{TOOL_NAME} doctor passed"));
        ansi::info(format!("version={VERSION}"));
        0
    } else {
        ansi::error(format!("{TOOL_NAME} doctor failed"));
        1
    }
}

pub fn print_error(message: impl AsRef<str>) {
    ansi::error(message);
}
