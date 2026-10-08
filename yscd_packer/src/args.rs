use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct Args {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub source_root: Option<PathBuf>,
    pub entry: Option<String>,
    pub overwrite: bool,
    pub debug: bool,
}

pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args::default();
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--input" | "-i" => {
                i += 1;
                out.input = Some(PathBuf::from(args.get(i).ok_or("--input requires value")?));
            }
            "--output" | "-o" | "--out-dir" => {
                i += 1;
                out.output = Some(PathBuf::from(args.get(i).ok_or("--output requires value")?));
            }
            "--source-root" => {
                i += 1;
                out.source_root = Some(PathBuf::from(
                    args.get(i).ok_or("--source-root requires value")?,
                ));
            }
            "--entry" | "--cue" => {
                i += 1;
                out.entry = Some(args.get(i).ok_or("--entry requires value")?.clone());
            }
            "--overwrite" | "-f" => out.overwrite = true,
            "--debug" | "--verbose" => out.debug = true,
            "--help" | "-h" => return Err("help requested".to_owned()),
            other if other.starts_with('-') => return Err(format!("unknown argument '{other}'")),
            positional => {
                if out.input.is_some() {
                    return Err(format!("unexpected positional argument '{positional}'"));
                }
                out.input = Some(PathBuf::from(positional));
            }
        }
        i += 1;
    }
    Ok(out)
}

pub fn required_input(cfg: &Args, command: &str, what: &str) -> Result<PathBuf, String> {
    cfg.input
        .clone()
        .ok_or_else(|| format!("{command} requires --input {what}"))
}

pub fn required_output(cfg: &Args, command: &str, what: &str) -> Result<PathBuf, String> {
    cfg.output
        .clone()
        .ok_or_else(|| format!("{command} requires --output {what}"))
}
