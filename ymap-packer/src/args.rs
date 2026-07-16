use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Args {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub root: PathBuf,
    pub logical_path: Option<String>,
    pub all: bool,
    pub dry_run: bool,
    pub debug: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            root: PathBuf::from("."),
            logical_path: None,
            all: false,
            dry_run: false,
            debug: false,
        }
    }
}

pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args::default();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--input" | "-i" => {
                index += 1;
                out.input = Some(PathBuf::from(value_at(args, index, "--input")?));
            }
            "--output" | "-o" => {
                index += 1;
                out.output = Some(PathBuf::from(value_at(args, index, "--output")?));
            }
            "--root" => {
                index += 1;
                out.root = PathBuf::from(value_at(args, index, "--root")?);
            }
            "--logical-path" => {
                index += 1;
                out.logical_path = Some(value_at(args, index, "--logical-path")?.to_owned());
            }
            "--all" => out.all = true,
            "--dry-run" | "--check" => out.dry_run = true,
            "--debug" | "--verbose" => out.debug = true,
            "--help" | "-h" | "--no-wait" => {}
            other => return Err(format!("unknown argument '{other}'")),
        }
        index += 1;
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

fn value_at<'a>(args: &'a [String], index: usize, option: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{option} requires value"))
}
