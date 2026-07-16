use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct CommonArgs {
    pub root: PathBuf,
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub all: bool,
    pub check: bool,
    pub logical_path: Option<String>,
    pub debug: bool,
}

pub fn parse_args(args: &[String]) -> Result<CommonArgs, String> {
    let mut out = CommonArgs {
        root: PathBuf::from("."),
        ..Default::default()
    };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                out.root = PathBuf::from(args.get(i).ok_or("--root requires value")?);
            }
            "--input" | "--xml" | "-i" => {
                i += 1;
                out.input = Some(PathBuf::from(args.get(i).ok_or("--input requires value")?));
            }
            "--output" | "-o" => {
                i += 1;
                out.output = Some(PathBuf::from(args.get(i).ok_or("--output requires value")?));
            }
            "--logical-path" => {
                i += 1;
                out.logical_path =
                    Some(args.get(i).ok_or("--logical-path requires value")?.clone());
            }
            "--all" => out.all = true,
            "--check" => out.check = true,
            "--debug" | "--verbose" => out.debug = true,
            "--help" | "-h" => return Err("help requested".to_owned()),
            other => return Err(format!("unknown argument '{other}'")),
        }
        i += 1;
    }
    Ok(out)
}

pub fn required_input(cfg: &CommonArgs) -> Result<PathBuf, String> {
    cfg.input
        .clone()
        .ok_or_else(|| "--input is required".to_owned())
}
