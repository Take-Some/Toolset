use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct Args {
    pub inputs: Vec<PathBuf>,
    pub output: Option<PathBuf>,
    pub out_dir: Option<PathBuf>,
    pub entry: Option<String>,
    pub family: Option<String>,
    pub style: Option<String>,
    pub weight: Option<u16>,
    pub no_compress: bool,
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
                out.inputs
                    .push(PathBuf::from(args.get(i).ok_or("--input requires value")?));
            }
            "--output" | "-o" => {
                i += 1;
                out.output = Some(PathBuf::from(args.get(i).ok_or("--output requires value")?));
            }
            "--out-dir" => {
                i += 1;
                out.out_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--out-dir requires value")?,
                ));
            }
            "--entry" | "--name" => {
                i += 1;
                out.entry = Some(args.get(i).ok_or("--entry requires value")?.clone());
            }
            "--family" => {
                i += 1;
                out.family = Some(args.get(i).ok_or("--family requires value")?.clone());
            }
            "--style" => {
                i += 1;
                out.style = Some(args.get(i).ok_or("--style requires value")?.clone());
            }
            "--weight" => {
                i += 1;
                out.weight = Some(
                    args.get(i)
                        .ok_or("--weight requires value")?
                        .parse::<u16>()
                        .map_err(|_| "--weight must be a number")?,
                );
            }
            "--no-compress" => out.no_compress = true,
            "--overwrite" => out.overwrite = true,
            "--debug" | "--verbose" => out.debug = true,
            "--help" | "-h" => return Err("help requested".to_owned()),
            other if other.starts_with('-') => return Err(format!("unknown argument '{other}'")),
            positional => out.inputs.push(PathBuf::from(positional)),
        }
        i += 1;
    }
    Ok(out)
}

pub fn required_sources(cfg: &Args, command: &str) -> Result<Vec<PathBuf>, String> {
    if cfg.inputs.is_empty() {
        return Err(format!("{command} requires one or more --input <font.ttf|font.otf|font.woff|font.woff2|font.ttc>"));
    }
    Ok(cfg.inputs.clone())
}

pub fn required_input(cfg: &Args, command: &str) -> Result<PathBuf, String> {
    if cfg.inputs.len() != 1 {
        return Err(format!(
            "{command} requires exactly one --input <file.neftd>; got {}",
            cfg.inputs.len()
        ));
    }
    Ok(cfg.inputs[0].clone())
}

pub fn required_output(cfg: &Args, command: &str, what: &str) -> Result<PathBuf, String> {
    cfg.output
        .clone()
        .ok_or_else(|| format!("{command} requires --output {what}"))
}
