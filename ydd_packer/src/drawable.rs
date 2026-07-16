use serde::Serialize;

pub const CONTENT_KIND_YDD_DRAWABLE_DICTIONARY: u16 = 2;

#[derive(Debug, Clone, Serialize)]
pub struct DrawableDictionary {
    pub models: Vec<DrawableModel>,
}

impl DrawableDictionary {
    pub fn new(models: Vec<DrawableModel>) -> Self {
        Self { models }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DrawableModel {
    pub name: String,
    pub source_path: String,
    pub properties_ref: Option<String>,
    pub meshes: Vec<DrawableMesh>,
    pub bounds: Bounds3,
}

#[derive(Debug, Clone, Serialize)]
pub struct DrawableMesh {
    pub name: String,
    pub material_ref: Option<String>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub bounds: Bounds3,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv0: [f32; 2],
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Bounds3 {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bounds3 {
    pub fn empty() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        }
    }

    pub fn include_point(&mut self, p: [f32; 3]) {
        for axis in 0..3 {
            self.min[axis] = self.min[axis].min(p[axis]);
            self.max[axis] = self.max[axis].max(p[axis]);
        }
    }

    pub fn include_bounds(&mut self, other: Bounds3) {
        self.include_point(other.min);
        self.include_point(other.max);
    }

    pub fn normalized(mut self) -> Self {
        for axis in 0..3 {
            if !self.min[axis].is_finite() || !self.max[axis].is_finite() {
                self.min[axis] = 0.0;
                self.max[axis] = 0.0;
            }
        }
        self
    }
}

pub fn recompute_mesh_bounds(vertices: &[Vertex]) -> Bounds3 {
    let mut b = Bounds3::empty();
    for v in vertices {
        b.include_point(v.position);
    }
    b.normalized()
}

pub fn recompute_model_bounds(meshes: &[DrawableMesh]) -> Bounds3 {
    let mut b = Bounds3::empty();
    for mesh in meshes {
        b.include_bounds(mesh.bounds);
    }
    b.normalized()
}

pub fn sanitize_entry_name(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if out.chars().last() != Some('_') || mapped != '_' {
            out.push(mapped);
        }
    }
    let out = out.trim_matches('_').to_owned();
    if out.is_empty() {
        "model".to_owned()
    } else {
        out
    }
}

pub fn stable_hash64(value: &str) -> u64 {
    let hash = blake3::hash(value.to_ascii_lowercase().as_bytes());
    u64::from_le_bytes(hash.as_bytes()[0..8].try_into().expect("hash prefix"))
}

pub fn validate_material_ref(value: &str) -> Result<(), String> {
    if !value.ends_with(".nemat") && !value.contains(".nemat@") {
        return Err(format!(
            "material ref '{value}' must point to .nemat or .nemat@entry"
        ));
    }
    Ok(())
}
