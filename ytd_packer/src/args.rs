use std::path::PathBuf;

#[derive(Default)]
pub struct Args {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub input_dir: Option<PathBuf>,
    pub textures: Vec<String>,
    pub entry: Option<String>,
    pub srgb: bool,
    pub linear_entries: Vec<String>,
    pub no_mips: bool,
    pub raw_data: bool,
    pub debug: bool,
}

pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        srgb: true,
        ..Default::default()
    };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--input" | "-i" => {
                i += 1;
                out.input = Some(PathBuf::from(args.get(i).ok_or("--input requires value")?));
            }
            "--output" | "-o" => {
                i += 1;
                out.output = Some(PathBuf::from(args.get(i).ok_or("--output requires value")?));
            }
            "--input-dir" => {
                i += 1;
                out.input_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--input-dir requires value")?,
                ));
            }
            "--texture" => {
                i += 1;
                out.textures.push(
                    args.get(i)
                        .ok_or("--texture requires name=path or path")?
                        .clone(),
                );
            }
            "--entry" => {
                i += 1;
                out.entry = Some(args.get(i).ok_or("--entry requires value")?.clone());
            }
            "--linear" => out.srgb = false,
            "--srgb" => out.srgb = true,
            "--linear-entry" => {
                i += 1;
                out.linear_entries.push(
                    args.get(i)
                        .ok_or("--linear-entry requires texture entry name")?
                        .clone(),
                );
            }
            "--no-mips" => out.no_mips = true,
            "--raw-data" => out.raw_data = true,
            "--debug" | "--verbose" => out.debug = true,
            "--help" | "-h" => return Err("help requested".to_owned()),
            other => return Err(format!("unknown argument '{other}'")),
        }
        i += 1;
    }
    Ok(out)
}

pub fn required_input(cfg: &Args, command: &str) -> Result<PathBuf, String> {
    cfg.input
        .clone()
        .ok_or_else(|| format!("{command} requires --input file.ytd"))
}

pub fn required_output(cfg: &Args, command: &str, what: &str) -> Result<PathBuf, String> {
    cfg.output
        .clone()
        .ok_or_else(|| format!("{command} requires --output {what}"))
}
