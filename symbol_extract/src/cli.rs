use std::path::PathBuf;

pub const TOOL_NAME: &str = "northstar-symbol-extract";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    Csv,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub inputs: Vec<PathBuf>,
    pub output: Option<PathBuf>,
    pub exclusions: Vec<String>,
    pub search_path: Option<PathBuf>,
    pub max_count: Option<usize>,
    pub format: OutputFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    AcceptedInputs,
    Doctor,
    Extract(Config),
}

pub fn usage() -> &'static str {
    "northstar-symbol-extract\n\n\
     Purpose:\n\
       List PE export symbols and COFF symbol-table names from executable/debug-symbol inputs.\n\n\
     Commands:\n\
       accepted-inputs        Print accepted input/output contract.\n\
       doctor                 Run a lightweight self-check.\n\
       version                Print tool version.\n\n\
     Usage:\n\
       northstar-symbol-extract -in[:type] filename [-out filename] [-exclude substring] [-count n]\n\
       northstar-symbol-extract -in filename --json\n\
       northstar-symbol-extract -in filename --csv\n\
       northstar-symbol-extract accepted-inputs\n\
       northstar-symbol-extract doctor\n\
       northstar-symbol-extract version\n\n\
     Options:\n\
       -in[:type] filename     Input PE/COFF file. Type suffix is accepted for legacy CLI compatibility.\n\
       -out filename           Write payload output to a file instead of stdout.\n\
       -exclude substring      Exclude symbols containing substring. May be repeated.\n\
       -searchpath path        Accepted for CLI compatibility; reserved for future PDB/DIA lookup.\n\
       -count n                Maximum number of payload records to emit.\n\
       --json                  Write JSON payload instead of text lines.\n\
       --csv                   Write CSV payload instead of text lines.\n\
       --help, -h, /?          Print this help.\n\n\
     Payload output:\n\
       Default extraction writes raw payload only: <symbol> 0x<RVA_HEX8> <source>.\n\
       Payload stdout never contains status tags, ANSI colors, progress or human commentary.\n\n\
     Examples:\n\
       northstar-symbol-extract -in game.exe\n\
       northstar-symbol-extract -in game.dll -exclude std:: -out .takesome\\symbols.txt\n\
       northstar-symbol-extract -in:exe game.exe -searchpath C:\\Symbols -out symbols.txt"
}

pub fn parse_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter().peekable();
    let mut cfg = Config {
        inputs: Vec::new(),
        output: None,
        exclusions: Vec::new(),
        search_path: None,
        max_count: None,
        format: OutputFormat::Text,
    };
    let mut saw_any = false;

    while let Some(arg) = iter.next() {
        saw_any = true;
        match arg.as_str() {
            "-h" | "--help" | "/?" | "help" => return Ok(Command::Help),
            "version" | "--version" | "-V" => return Ok(Command::Version),
            "accepted-inputs" | "--accepted-inputs" => return Ok(Command::AcceptedInputs),
            "doctor" | "--doctor" => return Ok(Command::Doctor),
            "--json" | "json" => cfg.format = OutputFormat::Json,
            "--csv" | "csv" => cfg.format = OutputFormat::Csv,
            "-out" => cfg.output = Some(next_path(&mut iter, "-out")?),
            "-exclude" => cfg.exclusions.push(next_value(&mut iter, "-exclude")?),
            "-searchpath" => cfg.search_path = Some(next_path(&mut iter, "-searchpath")?),
            "-count" => {
                let value = next_value(&mut iter, "-count")?;
                cfg.max_count = Some(value.parse::<usize>().map_err(|_| format!("invalid -count value '{}'", value))?);
            }
            value if value == "-in" || value.starts_with("-in:") => cfg.inputs.push(next_path(&mut iter, value)?),
            unknown if unknown.starts_with('-') => return Err(format!("unknown option '{}'", unknown)),
            filename => cfg.inputs.push(PathBuf::from(filename)),
        }
    }

    if !saw_any {
        return Ok(Command::Help);
    }
    if cfg.inputs.is_empty() {
        return Err("at least one input file must be specified".to_string());
    }
    Ok(Command::Extract(cfg))
}

fn next_value<I>(iter: &mut std::iter::Peekable<I>, option: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    iter.next().ok_or_else(|| format!("{} requires a value", option))
}

fn next_path<I>(iter: &mut std::iter::Peekable<I>, option: &str) -> Result<PathBuf, String>
where
    I: Iterator<Item = String>,
{
    Ok(PathBuf::from(next_value(iter, option)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_commands_parse() {
        assert_eq!(parse_args(vec!["version".into()]).unwrap(), Command::Version);
        assert_eq!(parse_args(vec!["accepted-inputs".into()]).unwrap(), Command::AcceptedInputs);
        assert_eq!(parse_args(vec!["doctor".into()]).unwrap(), Command::Doctor);
    }

    #[test]
    fn legacy_extract_args_parse() {
        let command = parse_args(vec![
            "-in:exe".into(),
            "a.exe".into(),
            "-exclude".into(),
            "std::".into(),
            "-out".into(),
            "s.txt".into(),
        ]).unwrap();
        assert_eq!(command, Command::Extract(Config {
            inputs: vec![PathBuf::from("a.exe")],
            output: Some(PathBuf::from("s.txt")),
            exclusions: vec!["std::".to_string()],
            search_path: None,
            max_count: None,
            format: OutputFormat::Text,
        }));
    }

    #[test]
    fn parses_payload_formats() {
        let json = parse_args(vec!["-in".into(), "a.exe".into(), "--json".into()]).unwrap();
        let csv = parse_args(vec!["-in".into(), "a.exe".into(), "--csv".into()]).unwrap();
        assert!(matches!(json, Command::Extract(Config { format: OutputFormat::Json, .. })));
        assert!(matches!(csv, Command::Extract(Config { format: OutputFormat::Csv, .. })));
    }
}
