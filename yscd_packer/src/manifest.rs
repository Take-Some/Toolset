use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST_SCHEMA: &str = "newengine.yscd.manifest.v1";
pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["wav", "ogg", "opus", "flac", "mp3"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DictionaryManifest {
    pub schema: String,
    pub version: u32,
    pub cues: Vec<CueManifest>,
}

impl Default for DictionaryManifest {
    fn default() -> Self {
        Self {
            schema: MANIFEST_SCHEMA.to_owned(),
            version: 1,
            cues: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CueManifest {
    pub name: String,
    #[serde(alias = "bus")]
    pub route: String,
    pub looping: bool,
    pub concurrency_group: String,
    pub concurrency_limit: usize,
    pub concurrency_scope: String,
    pub steal_rule: String,
    pub voice_budget: String,
    pub priority: i32,
    pub repeat_avoidance: usize,
    pub spatial_policy: String,
    pub gain_range: [f32; 2],
    pub pitch_range: [f32; 2],
    pub attenuation: Option<AttenuationManifest>,
    pub layers: Vec<LayerManifest>,
    pub sound_graph: Option<serde_json::Value>,
    pub clips: Vec<ClipManifest>,
}

impl Default for CueManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            route: String::new(),
            looping: false,
            concurrency_group: String::new(),
            concurrency_limit: 1,
            concurrency_scope: "global".to_owned(),
            steal_rule: "lower_priority_then_oldest".to_owned(),
            voice_budget: String::new(),
            priority: 0,
            repeat_avoidance: 0,
            spatial_policy: "inherit".to_owned(),
            gain_range: [1.0, 1.0],
            pitch_range: [1.0, 1.0],
            attenuation: None,
            layers: Vec::new(),
            sound_graph: None,
            clips: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AttenuationManifest {
    pub min_distance: f32,
    pub max_distance: f32,
    pub curve: String,
    pub rolloff: f32,
    pub curve_points: Vec<[f32; 2]>,
}

impl Default for AttenuationManifest {
    fn default() -> Self {
        Self {
            min_distance: 1.0,
            max_distance: 50.0,
            curve: "inverse".to_owned(),
            rolloff: 1.0,
            curve_points: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LayerManifest {
    pub name: String,
    pub role: String,
    pub clip_names: Vec<String>,
    pub gain: f32,
    pub pitch: f32,
    pub attenuation: Option<AttenuationManifest>,
}

impl Default for LayerManifest {
    fn default() -> Self {
        Self {
            name: "body".to_owned(),
            role: "body".to_owned(),
            clip_names: Vec::new(),
            gain: 1.0,
            pitch: 1.0,
            attenuation: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipManifest {
    pub name: String,
    pub source: String,
    pub weight: f32,
    pub gain: f32,
    pub pitch: f32,
}

impl Default for ClipManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            source: String::new(),
            weight: 1.0,
            gain: 1.0,
            pitch: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct CueDescriptor<'a> {
    route: &'a str,
    looping: bool,
    concurrency_group: &'a str,
    concurrency_limit: usize,
    concurrency_scope: &'a str,
    steal_rule: &'a str,
    voice_budget: &'a str,
    priority: i32,
    repeat_avoidance: usize,
    spatial_policy: &'a str,
    gain_range: [f32; 2],
    pitch_range: [f32; 2],
    attenuation: &'a Option<AttenuationManifest>,
    layers: &'a [LayerManifest],
    sound_graph: &'a Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedCueDescriptor {
    #[serde(default, alias = "bus")]
    pub route: String,
    pub looping: bool,
    pub concurrency_group: String,
    #[serde(default = "default_concurrency_limit")]
    pub concurrency_limit: usize,
    #[serde(default = "default_concurrency_scope")]
    pub concurrency_scope: String,
    #[serde(default = "default_steal_rule")]
    pub steal_rule: String,
    #[serde(default)]
    pub voice_budget: String,
    pub priority: i32,
    #[serde(default)]
    pub repeat_avoidance: usize,
    pub spatial_policy: String,
    pub gain_range: [f32; 2],
    pub pitch_range: [f32; 2],
    pub attenuation: Option<AttenuationManifest>,
    #[serde(default)]
    pub layers: Vec<LayerManifest>,
    #[serde(default)]
    pub sound_graph: Option<serde_json::Value>,
}


fn default_concurrency_limit() -> usize { 1 }
fn default_concurrency_scope() -> String { "global".to_owned() }
fn default_steal_rule() -> String { "lower_priority_then_oldest".to_owned() }

impl CueManifest {
    pub fn descriptor_json(&self) -> Result<String, String> {
        serde_json::to_string(&CueDescriptor {
            route: &self.route,
            looping: self.looping,
            concurrency_group: &self.concurrency_group,
            concurrency_limit: self.concurrency_limit,
            concurrency_scope: &self.concurrency_scope,
            steal_rule: &self.steal_rule,
            voice_budget: &self.voice_budget,
            priority: self.priority,
            repeat_avoidance: self.repeat_avoidance,
            spatial_policy: &self.spatial_policy,
            gain_range: self.gain_range,
            pitch_range: self.pitch_range,
            attenuation: &self.attenuation,
            layers: &self.layers,
            sound_graph: &self.sound_graph,
        })
        .map_err(|e| format!("serialize cue '{}' descriptor failed: {e}", self.name))
    }
}

pub fn load_manifest(path: &Path) -> Result<DictionaryManifest, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("read '{}' failed: {e}", path.display()))?;
    let manifest: DictionaryManifest = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse YSCD manifest '{}' failed: {e}", path.display()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest(manifest: &DictionaryManifest) -> Result<(), String> {
    if manifest.version != 1 {
        return Err(format!(
            "unsupported YSCD manifest version {}",
            manifest.version
        ));
    }
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(format!(
            "unsupported YSCD manifest schema '{}'; expected '{}'",
            manifest.schema, MANIFEST_SCHEMA
        ));
    }
    if manifest.cues.is_empty() {
        return Err("YSCD manifest requires at least one cue".to_owned());
    }
    let mut cue_names = BTreeSet::new();
    for cue in &manifest.cues {
        let name = cue.name.trim();
        if name.is_empty() || name.contains('@') {
            return Err(format!("invalid cue name '{}'", cue.name));
        }
        if !cue_names.insert(name.to_ascii_lowercase()) {
            return Err(format!("duplicate cue name '{}'", cue.name));
        }
        if cue.clips.is_empty() {
            return Err(format!("cue '{}' requires at least one clip", cue.name));
        }
        if cue.concurrency_limit == 0 {
            return Err(format!("cue '{}' concurrency_limit must be >= 1", cue.name));
        }
        if !matches!(cue.concurrency_scope.trim().to_ascii_lowercase().as_str(), "global" | "object" | "emitter") {
            return Err(format!("cue '{}' has unsupported concurrency_scope '{}'", cue.name, cue.concurrency_scope));
        }
        if cue.sound_graph.is_some() && !cue.layers.is_empty() {
            return Err(format!("cue '{}' cannot author both layers and sound_graph", cue.name));
        }
        validate_range(
            cue.gain_range,
            0.0,
            4.0,
            &format!("cue '{}' gain_range", cue.name),
        )?;
        validate_range(
            cue.pitch_range,
            0.05,
            4.0,
            &format!("cue '{}' pitch_range", cue.name),
        )?;
        if let Some(att) = &cue.attenuation {
            if !att.min_distance.is_finite()
                || !att.max_distance.is_finite()
                || att.min_distance < 0.0
                || att.max_distance <= att.min_distance
                || !att.rolloff.is_finite()
                || att.rolloff < 0.0
            {
                return Err(format!("cue '{}' has invalid attenuation", cue.name));
            }
            for point in &att.curve_points {
                if !point[0].is_finite()
                    || !point[1].is_finite()
                    || !(0.0..=1.0).contains(&point[0])
                    || !(0.0..=1.0).contains(&point[1])
                {
                    return Err(format!(
                        "cue '{}' has invalid attenuation curve point {:?}",
                        cue.name, point
                    ));
                }
            }
        }
        let mut clip_names = BTreeSet::new();
        for (index, clip) in cue.clips.iter().enumerate() {
            let source = normalize_relative_source(&clip.source)?;
            let ext = Path::new(&source)
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !SUPPORTED_AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                return Err(format!(
                    "cue '{}' clip '{}' source '{}' has unsupported audio extension '.{}'; supported={:?}",
                    cue.name, clip.name, source, ext, SUPPORTED_AUDIO_EXTENSIONS
                ));
            }
            let clip_name = effective_clip_name(clip, index);
            if !clip_names.insert(clip_name.to_ascii_lowercase()) {
                return Err(format!(
                    "cue '{}' has duplicate clip name '{}'",
                    cue.name, clip_name
                ));
            }
            if !clip.weight.is_finite() || clip.weight <= 0.0 {
                return Err(format!(
                    "cue '{}' clip '{}' weight must be > 0",
                    cue.name, clip_name
                ));
            }
            if !clip.gain.is_finite() || !(0.0..=4.0).contains(&clip.gain) {
                return Err(format!(
                    "cue '{}' clip '{}' gain out of range",
                    cue.name, clip_name
                ));
            }
            if !clip.pitch.is_finite() || !(0.05..=4.0).contains(&clip.pitch) {
                return Err(format!(
                    "cue '{}' clip '{}' pitch out of range",
                    cue.name, clip_name
                ));
            }
        }
        let mut layer_names = BTreeSet::new();
        for layer in &cue.layers {
            let layer_name = layer.name.trim();
            if layer_name.is_empty() || layer_name.contains('@') {
                return Err(format!(
                    "cue '{}' has invalid layer name '{}'",
                    cue.name, layer.name
                ));
            }
            if !layer_names.insert(layer_name.to_ascii_lowercase()) {
                return Err(format!(
                    "cue '{}' has duplicate layer name '{}'",
                    cue.name, layer.name
                ));
            }
            let role = layer.role.trim().to_ascii_lowercase();
            if !matches!(role.as_str(), "body" | "near" | "far" | "tail" | "aux" | "sweetener") {
                return Err(format!(
                    "cue '{}' layer '{}' has unsupported role '{}'",
                    cue.name, layer.name, layer.role
                ));
            }
            if layer.clip_names.is_empty() {
                return Err(format!(
                    "cue '{}' layer '{}' requires at least one clip name",
                    cue.name, layer.name
                ));
            }
            for clip_name in &layer.clip_names {
                if !clip_names.contains(&clip_name.trim().to_ascii_lowercase()) {
                    return Err(format!(
                        "cue '{}' layer '{}' references unknown clip '{}'",
                        cue.name, layer.name, clip_name
                    ));
                }
            }
            if !layer.gain.is_finite() || !(0.0..=4.0).contains(&layer.gain) {
                return Err(format!(
                    "cue '{}' layer '{}' gain out of range",
                    cue.name, layer.name
                ));
            }
            if !layer.pitch.is_finite() || !(0.05..=4.0).contains(&layer.pitch) {
                return Err(format!(
                    "cue '{}' layer '{}' pitch out of range",
                    cue.name, layer.name
                ));
            }
            if let Some(att) = &layer.attenuation {
                if !att.min_distance.is_finite()
                    || !att.max_distance.is_finite()
                    || att.min_distance < 0.0
                    || att.max_distance <= att.min_distance
                    || !att.rolloff.is_finite()
                    || att.rolloff < 0.0
                {
                    return Err(format!(
                        "cue '{}' layer '{}' has invalid attenuation",
                        cue.name, layer.name
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_range(range: [f32; 2], min: f32, max: f32, label: &str) -> Result<(), String> {
    if !range[0].is_finite()
        || !range[1].is_finite()
        || range[0] < min
        || range[1] > max
        || range[0] > range[1]
    {
        return Err(format!("{label} invalid [{}, {}]", range[0], range[1]));
    }
    Ok(())
}

pub fn normalize_relative_source(source: &str) -> Result<String, String> {
    let normalized = source.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("clip source cannot be empty".to_owned());
    }
    let path = Path::new(&normalized);
    if path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "clip source must be a portable relative path, got '{source}'"
        ));
    }
    Ok(normalized)
}

pub fn effective_clip_name(clip: &ClipManifest, index: usize) -> String {
    let trimmed = clip.name.trim();
    if !trimmed.is_empty() {
        return trimmed.to_owned();
    }
    Path::new(&clip.source)
        .file_stem()
        .and_then(|v| v.to_str())
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("clip_{index}"))
}

pub fn resolve_source(root: &Path, source: &str) -> Result<PathBuf, String> {
    let normalized = normalize_relative_source(source)?;
    Ok(root.join(normalized.replace('/', std::path::MAIN_SEPARATOR_STR)))
}
