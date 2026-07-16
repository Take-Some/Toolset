use crate::diagnostics;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    args::{parse_args, required_input, required_output, required_sources},
    drawable::{
        recompute_mesh_bounds, recompute_model_bounds, DrawableDictionary, DrawableMesh,
        DrawableModel, Vertex, CONTENT_KIND_YDD_DRAWABLE_DICTIONARY,
    },
    help, model, nef8,
};

const TOOL_NAME: &str = "northstar-ydd-packer";
const ACCEPTED_INPUTS: &str =
    "*.obj, *.gltf, *.glb, ASCII *.fbx model sources; *.ydd for inspect/list/validate/dump-body";
const PRODUCED_OUTPUTS: &str = "*.ydd runtime NEF8 drawable dictionary; *.yddbody body dumps";

pub fn dispatch(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        help::print_help();
        help::wait_for_enter();
        return Ok(());
    }
    if raw_args
        .iter()
        .skip(1)
        .any(|it| it == "--help" || it == "-h")
    {
        help::print_help();
        help::wait_for_enter();
        return Ok(());
    }
    match raw_args[0].as_str() {
        "pack" | "build" | "import" | "create" => run_pack(&raw_args[1..]),
        "inspect" | "parse" => run_inspect(&raw_args[1..]),
        "list" => run_list(&raw_args[1..]),
        "validate" => run_validate(&raw_args[1..]),
        "set-properties-ref" | "set-descriptor" => run_set_properties_ref_impl(&raw_args[1..]),
        "doctor" | "--doctor" => run_doctor(&raw_args[1..]),
        "dump-body" => run_dump_body(&raw_args[1..]),
        "accepted-inputs" | "--accepted-inputs" | "inputs" | "formats" => {
            diagnostics::print_contract(TOOL_NAME, ACCEPTED_INPUTS, PRODUCED_OUTPUTS);
            Ok(())
        }
        "version" | "--version" | "-V" => {
            diagnostics::print_version(TOOL_NAME);
            Ok(())
        }
        "help" | "--help" | "-h" => {
            help::print_help();
            help::wait_for_enter();
            Ok(())
        }
        other => Err(format!(
            "unknown command '{other}'. Use --help to list supported commands."
        )),
    }
}

fn run_pack(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let sources = required_sources(&cfg, "pack")?;
    let output = required_output(&cfg, "pack", "file.ydd")?;
    diagnostics::print_operation(
        TOOL_NAME,
        "pack",
        cfg.debug,
        ACCEPTED_INPUTS,
        PRODUCED_OUTPUTS,
    );
    diagnostics::print_debug_value(cfg.debug, "source_count", sources.len());
    for source in &sources {
        diagnostics::print_debug_value(cfg.debug, "source", source.display());
    }
    diagnostics::print_debug_value(cfg.debug, "output", output.display());
    let import_options = model::ImportOptions::from(&cfg);
    let dictionary = model::import_sources(&sources, &import_options)?;
    let logical = nef8::normalize_logical_path(&output.to_string_lossy());
    let bytes = nef8::pack_ydd(&dictionary, &logical)?;
    write_bytes(&output, &bytes)?;
    northstar_cli::ansi::ok(format!(
        "built resident YDD NEF8 ListFile: {} models={}",
        output.display(),
        dictionary.models.len()
    ));
    for model in &dictionary.models {
        northstar_cli::ansi::ok(format!(
            "entry {}@{} meshes={}",
            logical,
            model.name,
            model.meshes.len()
        ));
    }
    Ok(())
}

fn run_inspect(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "inspect")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let value = nef8::inspect_json(&bytes, &input.to_string_lossy())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn run_list(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "list")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let parsed = nef8::parse_ydd(&bytes, &input.to_string_lossy())?;
    for entry in parsed.entries {
        println!("{}", entry.selector);
    }
    Ok(())
}

fn run_validate(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "validate")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let parsed = nef8::parse_ydd(&bytes, &input.to_string_lossy())?;
    northstar_cli::ansi::ok(format!(
        "validated resident YDD NEF8 ListFile: {} models={}",
        input.display(),
        parsed.entries.len()
    ));
    Ok(())
}

fn run_doctor(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    if cfg.inputs.is_empty() {
        return run_self_doctor();
    }
    let input = required_input(&cfg, "doctor")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let parsed = nef8::parse_ydd(&bytes, &input.to_string_lossy())?;
    if !nef8::body_is_binary_ydd(&bytes)? {
        return Err(format!(
            "unsupported YDD body in '{}': expected newengine.ydd.binary_mesh; rebuild from authoring source",
            input.display()
        ));
    }
    println!(
        "[OK] YDD doctor passed: {} content_kind={} body=binary_mesh models={}",
        input.display(),
        parsed.header.content_kind,
        parsed.entries.len()
    );
    Ok(())
}

fn run_self_doctor() -> Result<(), String> {
    let vertices = vec![
        Vertex {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv0: [0.0, 0.0],
        },
        Vertex {
            position: [1.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv0: [1.0, 0.0],
        },
        Vertex {
            position: [0.0, 0.0, 1.0],
            normal: [0.0, 1.0, 0.0],
            uv0: [0.0, 1.0],
        },
    ];
    let mesh_bounds = recompute_mesh_bounds(&vertices);
    let mesh = DrawableMesh {
        name: "doctor_triangle".to_owned(),
        material_ref: None,
        vertices,
        indices: vec![0, 1, 2],
        bounds: mesh_bounds,
    };
    let model = DrawableModel {
        name: "doctor_triangle".to_owned(),
        source_path: "builtin://northstar-ydd-packer/doctor_triangle".to_owned(),
        bounds: recompute_model_bounds(std::slice::from_ref(&mesh)),
        properties_ref: Option::<String>::None,
        meshes: vec![mesh],
    };
    let dict = DrawableDictionary::new(vec![model]);
    let bytes = nef8::pack_ydd(&dict, "doctor/self_check.ydd")?;
    let parsed = nef8::parse_ydd(&bytes, "doctor/self_check.ydd")?;
    if parsed.header.content_kind as u16 != CONTENT_KIND_YDD_DRAWABLE_DICTIONARY {
        return Err(format!(
            "YDD self-doctor content_kind mismatch: actual={} expected={}",
            parsed.header.content_kind, CONTENT_KIND_YDD_DRAWABLE_DICTIONARY
        ));
    }
    if parsed.entries.len() != 1 {
        return Err(format!(
            "YDD self-doctor entry count mismatch: {}",
            parsed.entries.len()
        ));
    }
    diagnostics::print_doctor_ok(TOOL_NAME);
    println!(
        "[OK] YDD NEF8 self-check content_kind={} body=binary_mesh models={} mesh=triangle",
        parsed.header.content_kind,
        parsed.entries.len()
    );
    Ok(())
}

fn run_dump_body(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "dump-body")?;
    let output = required_output(&cfg, "dump-body", "file.yddbody")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let header = nef8::parse_header(&bytes)?;
    let body = nef8::decode_body(&bytes, &header)?;
    write_bytes(&output, &body)?;
    northstar_cli::ansi::ok(format!(
        "dumped resident drawable_dictionary body: {}",
        output.display()
    ));
    Ok(())
}

fn write_bytes(output: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create parent '{}' failed: {e}", parent.display()))?;
        }
    }
    fs::write(output, bytes).map_err(|e| format!("write '{}' failed: {e}", output.display()))
}

#[allow(dead_code)]
fn logical_asset_path(root: &Path, path: &Path) -> String {
    let absolute = if path.is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };
    let rel = absolute.strip_prefix(root).unwrap_or(&absolute);
    nef8::normalize_logical_path(&rel.to_string_lossy())
}

fn run_set_properties_ref_impl(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "set-properties-ref")?;
    let pr = cfg
        .properties_ref
        .as_deref()
        .ok_or("set-properties-ref requires --properties-ref <file.ytyp>")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let output = cfg.output.clone().unwrap_or_else(|| input.clone());
    let logical = nef8::normalize_logical_path(&output.to_string_lossy());
    let patched = nef8::set_properties_ref(&bytes, &logical, cfg.entry.as_deref(), pr)?;
    write_bytes(&output, &patched)?;
    northstar_cli::ansi::ok(format!(
        "set YDD properties_ref: {} -> {}",
        input.display(),
        pr
    ));
    Ok(())
}
