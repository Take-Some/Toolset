use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct Args {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub root: PathBuf,
    pub path: Option<String>,
    pub compare: Option<PathBuf>,
    pub overwrite: bool,
    pub no_compress: bool,
    pub debug: bool,
}

pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args { root: PathBuf::from("."), ..Default::default() };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--input" | "-i" | "--old" => { i += 1; out.input = Some(PathBuf::from(args.get(i).ok_or("--input requires value")?)); }
            "--output" | "-o" => { i += 1; out.output = Some(PathBuf::from(args.get(i).ok_or("--output requires value")?)); }
            "--compare" | "--other" | "--new" => { i += 1; out.compare = Some(PathBuf::from(args.get(i).ok_or("--compare requires value")?)); }
            "--root" => { i += 1; out.root = PathBuf::from(args.get(i).ok_or("--root requires value")?); }
            "--path" | "--entry" => { i += 1; out.path = Some(args.get(i).ok_or("--path requires value")?.clone()); }
            "--overwrite" => out.overwrite = true,
            "--no-compress" => out.no_compress = true,
            "--debug" | "--verbose" => out.debug = true,
            "--help" | "-h" => return Err("help requested".to_owned()),
            other if other.starts_with('-') => return Err(format!("unknown argument '{other}'")),
            positional => {
                if out.input.is_none() { out.input = Some(PathBuf::from(positional)); }
                else if out.compare.is_none() && out.output.is_none() { out.compare = Some(PathBuf::from(positional)); }
                else if out.output.is_none() { out.output = Some(PathBuf::from(positional)); }
                else { return Err(format!("unexpected positional argument '{positional}'")); }
            }
        }
        i += 1;
    }
    Ok(out)
}

pub fn required_input(cfg: &Args, command: &str) -> Result<PathBuf, String> {
    cfg.input.clone().ok_or_else(|| format!("{command} requires --input"))
}

pub fn required_output(cfg: &Args, command: &str) -> Result<PathBuf, String> {
    cfg.output.clone().ok_or_else(|| format!("{command} requires --output"))
}

pub fn required_compare(cfg: &Args, command: &str) -> Result<PathBuf, String> {
    cfg.compare.clone().ok_or_else(|| format!("{command} requires --compare"))
}
