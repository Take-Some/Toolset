use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct CommonArgs {
    pub root: PathBuf,
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub logical_path: Option<String>,
    pub entry: Option<String>,
    pub material: Option<String>,
    pub shader: Option<String>,
    pub blend: Option<String>,
    pub two_sided: bool,
    pub alpha_cutoff: Option<f32>,
    pub textures: Vec<String>,
    pub params: Vec<String>,
    pub pretty: bool,
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
            "--input" | "--json" | "-i" => {
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
            "--entry" => {
                i += 1;
                out.entry = Some(args.get(i).ok_or("--entry requires value")?.clone());
            }
            "--material" | "--name" => {
                i += 1;
                out.material = Some(args.get(i).ok_or("--material requires value")?.clone());
            }
            "--shader" => {
                i += 1;
                out.shader = Some(args.get(i).ok_or("--shader requires value")?.clone());
            }
            "--blend" => {
                i += 1;
                out.blend = Some(args.get(i).ok_or("--blend requires value")?.clone());
            }
            "--two-sided" => out.two_sided = true,
            "--alpha-cutoff" => {
                i += 1;
                let raw = args.get(i).ok_or("--alpha-cutoff requires value")?;
                out.alpha_cutoff = Some(
                    raw.parse::<f32>()
                        .map_err(|_| format!("invalid --alpha-cutoff '{raw}'"))?,
                );
            }
            "--texture" => {
                i += 1;
                out.textures.push(
                    args.get(i)
                        .ok_or("--texture requires slot=path.ytd@entry")?
                        .clone(),
                );
            }
            "--param" => {
                i += 1;
                out.params.push(
                    args.get(i)
                        .ok_or("--param requires name:type=value")?
                        .clone(),
                );
            }
            "--pretty" => out.pretty = true,
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

pub fn required_output(cfg: &CommonArgs, command: &str, what: &str) -> Result<PathBuf, String> {
    cfg.output
        .clone()
        .ok_or_else(|| format!("{command} requires --output {what}"))
}
