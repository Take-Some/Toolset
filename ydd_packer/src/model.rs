use std::fs;
use std::path::{Path, PathBuf};

use crate::args::Args;
use crate::drawable::{
    recompute_mesh_bounds, recompute_model_bounds, sanitize_entry_name, DrawableDictionary,
    DrawableMesh, DrawableModel, Vertex,
};

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub scale: f32,
    pub flip_v: bool,
    pub triangulate: bool,
    pub fallback_material: Option<String>,

    pub properties_ref: Option<String>,
    pub explicit_entry_name: Option<String>,
}

impl From<&Args> for ImportOptions {
    fn from(value: &Args) -> Self {
        Self {
            scale: value.scale,
            flip_v: value.flip_v,
            triangulate: value.triangulate,
            fallback_material: value.material.clone(),
            properties_ref: value.properties_ref.clone(),

            explicit_entry_name: value.entry.clone(),
        }
    }
}

pub fn import_sources(
    paths: &[PathBuf],
    options: &ImportOptions,
) -> Result<DrawableDictionary, String> {
    if paths.len() > 1 && options.explicit_entry_name.is_some() {
        return Err("--entry can only be used with a single source model; multi-model YDD names are derived from source files".to_owned());
    }
    let mut models = Vec::new();
    for path in expand_sources(paths)? {
        let mut imported = import_one(&path, options)?;
        for model in &mut imported {
            if model.properties_ref.is_none() {
                model.properties_ref = options.properties_ref.clone();
            }
        }
        models.append(&mut imported);
    }
    Ok(DrawableDictionary::new(models))
}

fn expand_sources(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            for entry in fs::read_dir(path)
                .map_err(|e| format!("read_dir '{}' failed: {e}", path.display()))?
            {
                let entry = entry.map_err(|e| e.to_string())?;
                let p = entry.path();
                if is_supported_source(&p) {
                    out.push(p);
                }
            }
        } else {
            out.push(path.clone());
        }
    }
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err("no supported source models found (.obj/.gltf/.glb/.fbx)".to_owned());
    }
    Ok(out)
}

fn import_one(path: &Path, options: &ImportOptions) -> Result<Vec<DrawableModel>, String> {
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "obj" => Ok(vec![crate::obj::import_obj(path, options)?]),
        "gltf" => crate::gltf::import_gltf(path, options),
        "glb" => crate::gltf::import_glb(path, options),
        "fbx" => crate::fbx::import_fbx(path, options),
        _ => Err(format!(
            "unsupported model source '{}'; expected .obj/.gltf/.glb/.fbx",
            path.display()
        )),
    }
}

pub fn is_supported_source(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|x| x.to_str())
            .map(|x| x.to_ascii_lowercase())
            .as_deref(),
        Some("obj" | "gltf" | "glb" | "fbx")
    )
}

pub fn source_entry_name(path: &Path, options: &ImportOptions) -> String {
    if let Some(explicit) = &options.explicit_entry_name {
        return sanitize_entry_name(explicit);
    }
    let stem = path.file_stem().and_then(|x| x.to_str()).unwrap_or("model");
    sanitize_entry_name(stem)
}

pub fn make_mesh(
    name: String,
    material_ref: Option<String>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
) -> DrawableMesh {
    let bounds = recompute_mesh_bounds(&vertices);
    DrawableMesh {
        name,
        material_ref,
        vertices,
        indices,
        bounds,
    }
}

pub fn make_model(name: String, source_path: &Path, meshes: Vec<DrawableMesh>) -> DrawableModel {
    let bounds = recompute_model_bounds(&meshes);
    DrawableModel {
        name,
        source_path: source_path.to_string_lossy().replace('\\', "/"),
        properties_ref: None,
        meshes,
        bounds,
    }
}

pub fn apply_position_scale(mut p: [f32; 3], scale: f32) -> [f32; 3] {
    p[0] *= scale;
    p[1] *= scale;
    p[2] *= scale;
    p
}

pub fn default_normal() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}
