use crate::hash::joaat_hash;
use std::path::Path;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub strip_ext: bool,
    pub literal: bool,
    pub filename: String,
    pub target_hash: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    AcceptedInputs,
    Doctor,
    Hash(Config),
}

pub fn usage() -> &'static str {
    "northstar-hasher\n\n\
     Purpose:\n\
       Hash newline-separated name lists with a RAGE/JOAAT-compatible 32-bit string hash.\n\n\
     Commands:\n\
       accepted-inputs        Print accepted input/output contract.\n\
       doctor                 Run a lightweight self-check.\n\
       version                Print tool version.\n\n\
     Usage:\n\
       northstar-hasher [-stripext] [-literal] filename [hashcode]\n\
       northstar-hasher accepted-inputs\n\
       northstar-hasher doctor\n\
       northstar-hasher version\n\n\
     Arguments:\n\
       filename    Text file with one name per line.\n\
       hashcode    Optional hexadecimal hash filter, for example 0xb779a091.\n\n\
     Options:\n\
       -stripext     Strip the final file extension before hashing.\n\
       -literal      Hash bytes exactly as written; default lowercases ASCII.\n\
       --help, -h    Print this help.\n\
       --version     Print version.\n\n\
     Examples:\n\
       northstar-hasher names.txt\n\
       northstar-hasher -literal names.txt\n\
       northstar-hasher -stripext names.txt\n\
       northstar-hasher names.txt 0xb779a091"
}

pub fn accepted_inputs_text() -> String {
    format!(
        "[INFO] northstar-hasher version={}\n\
         [INFO] production output: compact ATSTRINGHASH/ATLITERALSTRINGHASH lines for suite logs\n\
         [INFO] debug output: add --help for usage details\n\
         [INFO] accepted input files: *.txt newline-separated name lists\n\
         [INFO] produced output files: stdout ATSTRINGHASH/ATLITERALSTRINGHASH lines",
        VERSION
    )
}

pub fn doctor_text() -> String {
    let lower = joaat_hash("Adder", false) == joaat_hash("adder", false);
    let literal = joaat_hash("Adder", true) != joaat_hash("adder", true);
    if lower && literal {
        format!("[OK] northstar-hasher doctor passed\n[INFO] version={}", VERSION)
    } else {
        "[ERROR] northstar-hasher doctor failed".to_string()
    }
}

pub fn parse_hex_hash(value: &str) -> Result<u32, String> {
    let trimmed = value.trim();
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    u32::from_str_radix(hex, 16).map_err(|_| format!("invalid hashcode '{}'", value))
}

pub fn strip_extension(name: &str) -> &str {
    Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_else(|| match name.rfind('.') {
            Some(dot) => &name[..dot],
            None => name,
        })
}

pub fn output_line(name: &str, literal: bool) -> String {
    let hash = joaat_hash(name, literal);
    if literal {
        format!("ATLITERALSTRINGHASH(\"{}\",0x{:x})", name, hash)
    } else {
        format!("ATSTRINGHASH(\"{}\",0x{:x})", name, hash)
    }
}

pub fn parse_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut strip_ext = false;
    let mut literal = false;
    let mut positional: Vec<String> = Vec::new();
    let mut saw_any = false;

    for arg in args {
        saw_any = true;
        match arg.as_str() {
            "-stripext" => strip_ext = true,
            "-literal" => literal = true,
            "-h" | "--help" | "/?" | "help" => return Ok(Command::Help),
            "accepted-inputs" | "--accepted-inputs" => return Ok(Command::AcceptedInputs),
            "doctor" | "--doctor" => return Ok(Command::Doctor),
            "version" | "--version" | "-V" => return Ok(Command::Version),
            unknown if unknown.starts_with('-') => return Err(format!("unknown option '{}'", unknown)),
            _ => positional.push(arg),
        }
    }

    if !saw_any {
        return Ok(Command::Help);
    }

    if positional.is_empty() || positional.len() > 2 {
        return Err(usage().to_string());
    }

    let filename = positional[0].clone();
    let target_hash = if positional.len() == 2 {
        Some(parse_hex_hash(&positional[1])?)
    } else {
        None
    };

    Ok(Command::Hash(Config { strip_ext, literal, filename, target_hash }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_returns_help_command() { assert_eq!(parse_args(Vec::<String>::new()).unwrap(), Command::Help); }
    #[test]
    fn parses_service_commands() {
        assert_eq!(parse_args(vec!["accepted-inputs".into()]).unwrap(), Command::AcceptedInputs);
        assert_eq!(parse_args(vec!["doctor".into()]).unwrap(), Command::Doctor);
        assert_eq!(parse_args(vec!["version".into()]).unwrap(), Command::Version);
    }
    #[test]
    fn parses_hash_command() {
        let command = parse_args(vec!["-stripext".into(), "names.txt".into(), "0xdeadbeef".into()]).unwrap();
        assert_eq!(command, Command::Hash(Config { strip_ext: true, literal: false, filename: "names.txt".into(), target_hash: Some(0xdeadbeef) }));
    }
    #[test]
    fn strip_ext_removes_final_extension() { assert_eq!(strip_extension("archive.name.ytd"), "archive.name"); }
    #[test]
    fn output_uses_legacy_macro_names() {
        assert!(output_line("adder", false).starts_with("ATSTRINGHASH("));
        assert!(output_line("Adder", true).starts_with("ATLITERALSTRINGHASH("));
    }
}
