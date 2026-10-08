use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct Args {
    pub inputs: Vec<PathBuf>,
    pub output: Option<PathBuf>,
    pub out_dir: Option<PathBuf>,
    pub entry: Option<String>,
    pub mesh: Option<String>,
    pub material: Option<String>,
    pub material_file: Option<PathBuf>,
    pub properties_ref: Option<String>,
    pub scale: f32,
    pub flip_v: bool,
    pub triangulate: bool,
    pub include_diagnostics: bool,
    pub debug: bool,
    pub fix: bool,
}

pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        scale: 1.0,
        triangulate: true,
        ..Default::default()
    };
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
            "--mesh" => {
                i += 1;
                out.mesh = Some(args.get(i).ok_or("--mesh requires value")?.clone());
            }
            "--material" => {
                i += 1;
                out.material = Some(args.get(i).ok_or("--material requires value")?.clone());
            }
            "--material-file" => {
                i += 1;
                out.material_file = Some(PathBuf::from(
                    args.get(i).ok_or("--material-file requires value")?,
                ));
            }
            "--properties-ref" | "--descriptor-ref" => {
                i += 1;
                out.properties_ref = Some(
                    args.get(i)
                        .ok_or("--properties-ref requires value")?
                        .clone(),
                );
            }
            "--scale" => {
                i += 1;
                out.scale = args
                    .get(i)
                    .ok_or("--scale requires value")?
                    .parse::<f32>()
                    .map_err(|_| "--scale must be a number".to_owned())?;
            }
            "--flip-v" => out.flip_v = true,
            "--no-triangulate" => out.triangulate = false,
            "--include-diagnostics" => out.include_diagnostics = true,
            "--debug" | "--verbose" => out.debug = true,
            "--fix" => out.fix = true,
            "--help" | "-h" => return Err("help requested".to_owned()),
            other if other.starts_with('-') => return Err(format!("unknown argument '{other}'")),
            positional => out.inputs.push(PathBuf::from(positional)),
        }
        i += 1;
    }
    Ok(out)
}

pub fn required_input(cfg: &Args, command: &str) -> Result<PathBuf, String> {
    if cfg.inputs.len() != 1 {
        return Err(format!(
            "{command} requires exactly one --input <file.ydd>; got {}",
            cfg.inputs.len()
        ));
    }
    Ok(cfg.inputs[0].clone())
}

pub fn required_sources(cfg: &Args, command: &str) -> Result<Vec<PathBuf>, String> {
    if cfg.inputs.is_empty() {
        return Err(format!(
            "{command} requires one or more --input <model.obj|model.gltf|model.glb|model.fbx>"
        ));
    }
    Ok(cfg.inputs.clone())
}

pub fn required_output(cfg: &Args, command: &str, what: &str) -> Result<PathBuf, String> {
    cfg.output
        .clone()
        .ok_or_else(|| format!("{command} requires --output {what}"))
}
