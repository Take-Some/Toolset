use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use crate::args::CommonArgs;
use crate::nef8;

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialLibrary {
    pub schema: String,
    pub materials: Vec<MaterialEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialEntry {
    pub name: String,
    pub shader: String,
    pub domain: String,
    pub shading_model: String,
    pub blend: String,
    pub two_sided: bool,
    pub alpha_cutoff: Option<f32>,
    pub textures: Vec<TextureBinding>,
    pub params: Vec<ParamBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureBinding {
    pub slot: String,
    pub reference: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParamBinding {
    pub name: String,
    pub ty: String,
    pub value: String,
}

#[cfg(test)]
pub fn xml_from_draft_args(cfg: &CommonArgs) -> Result<String, String> {
    let name = cfg
        .material
        .clone()
        .unwrap_or_else(|| "material".to_owned());
    let shader = cfg
        .shader
        .clone()
        .unwrap_or_else(|| "pbr.default".to_owned());
    let blend = cfg.blend.clone().unwrap_or_else(|| "opaque".to_owned());
    let two_sided = if cfg.two_sided { "true" } else { "false" };
    let alpha_cutoff_attr = cfg
        .alpha_cutoff
        .map(|v| format!(" alpha_cutoff=\"{}\"", xml_escape(&v.to_string())))
        .unwrap_or_default();

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n");
    xml.push_str("<NematMaterialLibrary schema=\"newengine.nemat.xmltype.v1\">\n");
    xml.push_str("  <Entries>\n");
    xml.push_str(&format!("    <Material name=\"{}\" shader=\"{}\" domain=\"surface\" shading_model=\"pbr_metallic_roughness\">\n", xml_escape(&name), xml_escape(&shader)));
    xml.push_str(&format!(
        "      <Surface blend=\"{}\" two_sided=\"{}\"{} />\n",
        xml_escape(&blend),
        two_sided,
        alpha_cutoff_attr
    ));
    if !cfg.textures.is_empty() {
        xml.push_str("      <Textures>\n");
        for raw in &cfg.textures {
            let (slot, reference) = split_key_value(raw, "--texture", "slot=path.ytd@entry")?;
            validate_ytd_entry_ref(&reference)
                .map_err(|e| format!("texture slot '{slot}' invalid: {e}"))?;
            xml.push_str(&format!(
                "        <Texture slot=\"{}\" ref=\"{}\" required=\"true\" />\n",
                xml_escape(&slot),
                xml_escape(&reference)
            ));
        }
        xml.push_str("      </Textures>\n");
    }
    if !cfg.params.is_empty() {
        xml.push_str("      <Params>\n");
        for raw in &cfg.params {
            let (name_type, value) = split_key_value(raw, "--param", "name:type=value")?;
            let (param_name, ty) = split_name_type(&name_type)?;
            validate_param_value(&ty, &value)?;
            xml.push_str(&format!(
                "        <Param name=\"{}\" type=\"{}\" value=\"{}\" />\n",
                xml_escape(&param_name),
                xml_escape(&ty),
                xml_escape(&value)
            ));
        }
        xml.push_str("      </Params>\n");
    }
    xml.push_str("    </Material>\n");
    xml.push_str("  </Entries>\n");
    xml.push_str("</NematMaterialLibrary>\n");
    validate_material_xml(&xml)?;
    Ok(xml)
}

pub fn parse_material_xml(xml: &str) -> Result<MaterialLibrary, String> {
    validate_material_xml(xml)?;
    let schema = attr_value_in_first_tag(xml, "NematMaterialLibrary", "schema")
        .unwrap_or_else(|| "newengine.nemat.xmltype.v1".to_owned());
    let mut materials = Vec::new();
    for tag in open_tags(xml, "Material") {
        let name = attr_value(&tag, "name").ok_or_else(|| "<Material> missing name".to_owned())?;
        let shader = attr_value(&tag, "shader").unwrap_or_else(|| "pbr.default".to_owned());
        let domain = attr_value(&tag, "domain").unwrap_or_else(|| "surface".to_owned());
        let shading_model = attr_value(&tag, "shading_model")
            .unwrap_or_else(|| "pbr_metallic_roughness".to_owned());
        let body = element_body(xml, "Material", &name).unwrap_or_default();
        let surface = open_tags(&body, "Surface").into_iter().next();
        let blend = surface
            .as_deref()
            .and_then(|t| attr_value(t, "blend"))
            .unwrap_or_else(|| "opaque".to_owned());
        let two_sided = surface
            .as_deref()
            .and_then(|t| attr_value(t, "two_sided"))
            .as_deref()
            == Some("true");
        let alpha_cutoff = surface
            .as_deref()
            .and_then(|t| attr_value(t, "alpha_cutoff"))
            .map(|raw| {
                raw.parse::<f32>()
                    .map_err(|_| format!("material '{name}' invalid alpha_cutoff '{raw}'"))
            })
            .transpose()?;
        let textures = open_tags(&body, "Texture")
            .into_iter()
            .map(|texture_tag| {
                let slot = attr_value(&texture_tag, "slot")
                    .ok_or_else(|| format!("material '{name}' <Texture> missing slot"))?;
                let reference = attr_value(&texture_tag, "ref")
                    .ok_or_else(|| format!("material '{name}' texture '{slot}' missing ref"))?;
                validate_ytd_entry_ref(&reference)
                    .map_err(|e| format!("material '{name}' texture slot '{slot}' invalid: {e}"))?;
                let required = attr_value(&texture_tag, "required")
                    .map(|v| v != "false")
                    .unwrap_or(true);
                Ok(TextureBinding {
                    slot,
                    reference,
                    required,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let params = open_tags(&body, "Param")
            .into_iter()
            .map(|param_tag| {
                let param_name = attr_value(&param_tag, "name")
                    .ok_or_else(|| format!("material '{name}' <Param> missing name"))?;
                let ty = attr_value(&param_tag, "type").unwrap_or_else(|| "float".to_owned());
                let value = attr_value(&param_tag, "value").ok_or_else(|| {
                    format!("material '{name}' param '{param_name}' missing value")
                })?;
                validate_param_value(&ty, &value)?;
                if ty == "texture_ref" {
                    validate_ytd_entry_ref(&value).map_err(|e| {
                        format!("material '{name}' param '{param_name}' texture ref invalid: {e}")
                    })?;
                }
                Ok(ParamBinding {
                    name: param_name,
                    ty,
                    value,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        materials.push(MaterialEntry {
            name,
            shader,
            domain,
            shading_model,
            blend,
            two_sided,
            alpha_cutoff,
            textures,
            params,
        });
    }
    let library = MaterialLibrary { schema, materials };
    validate_library(&library)?;
    Ok(library)
}

pub fn validate_material_xml(xml: &str) -> Result<Vec<String>, String> {
    let root = root_name(xml).ok_or_else(|| "XML document has no root element".to_owned())?;
    if root != "NematMaterialLibrary" {
        return Err(format!(
            "expected <NematMaterialLibrary> root, got <{root}>"
        ));
    }
    let material_count = count_open_tags(xml, "Material");
    if material_count == 0 {
        return Err("NematMaterialLibrary has no <Material> entries".to_owned());
    }
    let _ = parse_material_xml_without_root_reentry(xml)?;
    Ok(Vec::new())
}

fn parse_material_xml_without_root_reentry(xml: &str) -> Result<(), String> {
    let mut names = BTreeSet::new();
    let mut hashes = BTreeMap::<u64, String>::new();
    for tag in open_tags(xml, "Material") {
        let name = attr_value(&tag, "name").ok_or_else(|| "<Material> missing name".to_owned())?;
        if name.trim().is_empty() {
            return Err("material entry has empty name".to_owned());
        }
        let lowered = name.to_ascii_lowercase();
        if !names.insert(lowered) {
            return Err(format!("duplicate material name '{name}'"));
        }
        let hash = nef8::stable_u64(&name);
        if let Some(existing) = hashes.insert(hash, name.clone()) {
            return Err(format!(
                "stable material hash collision between '{existing}' and '{name}'"
            ));
        }
    }
    for tag in open_tags(xml, "Texture") {
        let slot = attr_value(&tag, "slot").ok_or_else(|| "<Texture> missing slot".to_owned())?;
        let reference = attr_value(&tag, "ref")
            .ok_or_else(|| format!("<Texture slot='{slot}'> missing ref"))?;
        validate_ytd_entry_ref(&reference)
            .map_err(|e| format!("texture slot '{slot}' invalid: {e}"))?;
    }
    for tag in open_tags(xml, "Param") {
        let ty = attr_value(&tag, "type").unwrap_or_else(|| "float".to_owned());
        let value = attr_value(&tag, "value").ok_or_else(|| "<Param> missing value".to_owned())?;
        validate_param_value(&ty, &value)?;
        if ty == "texture_ref" {
            validate_ytd_entry_ref(&value)
                .map_err(|e| format!("texture_ref param invalid: {e}"))?;
        }
    }
    for tag in open_tags(xml, "Surface") {
        let blend = attr_value(&tag, "blend").unwrap_or_else(|| "opaque".to_owned());
        validate_blend(&blend)?;
        if let Some(alpha) = attr_value(&tag, "alpha_cutoff") {
            let parsed = alpha
                .parse::<f32>()
                .map_err(|_| format!("invalid alpha_cutoff '{alpha}'"))?;
            if !(0.0..=1.0).contains(&parsed) {
                return Err(format!(
                    "alpha_cutoff must be between 0 and 1, got {parsed}"
                ));
            }
        }
    }
    Ok(())
}

pub fn manifest_json(xml: &str, logical_path: &str) -> Result<Value, String> {
    let library = parse_material_xml(xml)?;
    let logical = nef8::normalize_logical_path(logical_path);
    let entries = library
        .materials
        .iter()
        .map(|material| {
            let entry_ref = format!("{}@{}", logical, material.name);
            let dependencies = material_texture_dependencies(material)
                .into_iter()
                .map(|(slot, reference, required)| {
                    json!({
                        "reference": reference,
                        "kind": format!("texture/{slot}"),
                        "role": format!("texture/{slot}"),
                        "domain": "engine.assets",
                        "required": required,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "name": material.name,
                "stable_id": format!("{:016x}", nef8::stable_u64(&material.name)),
                "asset_kind": "material",
                "entry_ref": entry_ref,
                "route": {
                    "gateway": "engine.materials",
                    "method": "materials.load_descriptor_v1",
                    "semantic_owner": "material"
                },
                "shader": material.shader,
                "surface": {
                    "blend": material.blend,
                    "two_sided": material.two_sided,
                    "alpha_cutoff": material.alpha_cutoff,
                },
                "dependencies": dependencies,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema": "newengine.asset.list_files.v1",
        "source": logical,
        "file_kind": "material_library",
        "container": "newengine.listfile.nef8.nemat",
        "codec": "asset.codec.listfile.nemat",
        "semantic_gateway": "engine.materials",
        "body": "XMLtype",
        "entries": entries,
        "policy": [
            "authoring source is XMLtype <NematMaterialLibrary>",
            "runtime .nemat is NEF8/ListFile content_kind=material_library",
            "entries are addressed as <logical-path>@entry",
            "texture dependencies must be .ytd@entry selectors",
            "renderer receives RenderMaterialPacket and never parses .nemat directly"
        ]
    }))
}

pub fn graph_json(xml: &str, logical_path: &str) -> Result<Value, String> {
    let library = parse_material_xml(xml)?;
    let logical = nef8::normalize_logical_path(logical_path);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut texture_nodes = BTreeSet::new();
    nodes.push(json!({"id": logical, "role": "material_library", "domain": "engine.materials"}));
    for material in &library.materials {
        let material_ref = format!("{}@{}", logical, material.name);
        nodes.push(json!({"id": material_ref, "role": "material", "domain": "engine.materials", "shader": material.shader}));
        edges.push(json!({"from": logical, "to": material_ref, "kind": "contains_material", "required": true}));
        for (slot, reference, required) in material_texture_dependencies(material) {
            if texture_nodes.insert(reference.clone()) {
                nodes.push(json!({"id": reference, "role": "texture_dictionary_entry", "domain": "engine.assets"}));
            }
            edges.push(
                json!({"from": material_ref, "to": reference, "kind": slot, "required": required}),
            );
        }
    }
    Ok(
        json!({"schema": "northstar.nemat.asset_graph.v1", "source": logical, "nodes": nodes, "edges": edges}),
    )
}

pub fn summary_json(xml: &str, logical_path: &str) -> Result<Value, String> {
    let library = parse_material_xml(xml)?;
    let logical = nef8::normalize_logical_path(logical_path);
    Ok(json!({
        "schema": "northstar.nemat.xmltype.summary.v1",
        "logical_path": logical,
        "xml_root": "NematMaterialLibrary",
        "xml_schema": library.schema,
        "material_count": library.materials.len(),
        "materials": library.materials.iter().map(|material| json!({
            "name": material.name,
            "entry_ref": format!("{}@{}", logical, material.name),
            "shader": material.shader,
            "domain": material.domain,
            "shading_model": material.shading_model,
            "surface": {"blend": material.blend, "two_sided": material.two_sided, "alpha_cutoff": material.alpha_cutoff},
            "texture_count": material.textures.len(),
            "param_count": material.params.len(),
        })).collect::<Vec<_>>()
    }))
}

fn validate_library(library: &MaterialLibrary) -> Result<(), String> {
    if library.materials.is_empty() {
        return Err("material library contains no materials".to_owned());
    }
    let mut names = BTreeSet::new();
    let mut hashes = BTreeMap::<u64, String>::new();
    for material in &library.materials {
        if material.name.trim().is_empty() {
            return Err("material entry has empty name".to_owned());
        }
        let lowered = material.name.to_ascii_lowercase();
        if !names.insert(lowered) {
            return Err(format!("duplicate material name '{}'", material.name));
        }
        let hash = nef8::stable_u64(&material.name);
        if let Some(existing) = hashes.insert(hash, material.name.clone()) {
            return Err(format!(
                "stable material hash collision between '{}' and '{}'",
                existing, material.name
            ));
        }
        validate_blend(&material.blend)?;
        if material
            .alpha_cutoff
            .is_some_and(|v| !(0.0..=1.0).contains(&v))
        {
            return Err(format!(
                "material '{}' alpha_cutoff must be between 0 and 1",
                material.name
            ));
        }
        for texture in &material.textures {
            validate_ytd_entry_ref(&texture.reference).map_err(|e| {
                format!(
                    "material '{}' texture slot '{}' invalid: {e}",
                    material.name, texture.slot
                )
            })?;
        }
        for param in &material.params {
            validate_param_value(&param.ty, &param.value)?;
            if param.ty == "texture_ref" {
                validate_ytd_entry_ref(&param.value).map_err(|e| {
                    format!(
                        "material '{}' param '{}' texture ref invalid: {e}",
                        material.name, param.name
                    )
                })?;
            }
        }
    }
    Ok(())
}

fn material_texture_dependencies(material: &MaterialEntry) -> Vec<(String, String, bool)> {
    let mut out = material
        .textures
        .iter()
        .map(|t| (t.slot.clone(), t.reference.clone(), t.required))
        .collect::<Vec<_>>();
    for param in &material.params {
        if param.ty == "texture_ref" {
            out.push((format!("param/{}", param.name), param.value.clone(), true));
        }
    }
    out
}

pub fn validate_ytd_entry_ref(reference: &str) -> Result<(), String> {
    let value = reference.trim().replace('\\', "/");
    let lowered = value.to_ascii_lowercase();
    if lowered.contains(".neytd") {
        return Err(
            "material texture references must use .ytd@entry; .neytd is not canonical".to_owned(),
        );
    }
    for ext in [".png", ".jpg", ".jpeg", ".tga", ".bmp", ".webp", ".dds"] {
        if lowered.ends_with(ext) || lowered.contains(&format!("{ext}@")) {
            return Err("material texture references must use .ytd@entry; raw/source texture files are importer inputs only".to_owned());
        }
    }
    let Some((path, entry)) = value.split_once('@') else {
        return Err("missing @entry selector".to_owned());
    };
    if !path.to_ascii_lowercase().ends_with(".ytd") {
        return Err(format!(
            "expected .ytd texture dictionary path, got '{path}'"
        ));
    }
    if entry.trim().is_empty() {
        return Err("empty texture entry selector".to_owned());
    }
    Ok(())
}

fn validate_blend(value: &str) -> Result<(), String> {
    const BLENDS: &[&str] = &["opaque", "masked", "alpha", "additive"];
    if BLENDS.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "invalid blend '{}'; expected one of {:?}",
            value, BLENDS
        ))
    }
}

fn validate_param_value(ty: &str, value: &str) -> Result<(), String> {
    match ty {
        "float" => parse_f32(value).map(|_| ()),
        "float2" => parse_array::<2>(value).map(|_| ()),
        "float3" => parse_array::<3>(value).map(|_| ()),
        "float4" | "color" => parse_array::<4>(value).map(|_| ()),
        "int" => value
            .parse::<i32>()
            .map(|_| ())
            .map_err(|_| format!("invalid int param value '{value}'")),
        "bool" => value
            .parse::<bool>()
            .map(|_| ())
            .map_err(|_| format!("invalid bool param value '{value}'")),
        "enum" => {
            if value.trim().is_empty() {
                Err("enum param value cannot be empty".to_owned())
            } else {
                Ok(())
            }
        }
        "texture_ref" => validate_ytd_entry_ref(value),
        other => Err(format!("unknown param type '{other}'")),
    }
}

fn parse_array<const N: usize>(value: &str) -> Result<[f32; N], String> {
    let parts = value
        .split(',')
        .map(|v| parse_f32(v.trim()))
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != N {
        return Err(format!(
            "expected {N} comma-separated float values, got {}",
            parts.len()
        ));
    }
    let mut out = [0.0f32; N];
    out.copy_from_slice(&parts);
    Ok(out)
}

fn parse_f32(value: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|_| format!("invalid float value '{value}'"))
}

#[cfg(test)]
fn split_key_value(raw: &str, flag: &str, expected: &str) -> Result<(String, String), String> {
    let (key, value) = raw
        .split_once('=')
        .ok_or_else(|| format!("{flag} expects {expected}, got '{raw}'"))?;
    if key.trim().is_empty() || value.trim().is_empty() {
        return Err(format!("{flag} expects non-empty {expected}, got '{raw}'"));
    }
    Ok((key.trim().to_owned(), value.trim().to_owned()))
}

#[cfg(test)]
fn split_name_type(raw: &str) -> Result<(String, String), String> {
    let (name, ty) = raw
        .split_once(':')
        .ok_or_else(|| format!("--param expects name:type=value, got '{raw}=...'"))?;
    if name.trim().is_empty() || ty.trim().is_empty() {
        return Err(format!(
            "--param expects non-empty name/type, got '{raw}=...'"
        ));
    }
    Ok((name.trim().to_owned(), ty.trim().to_owned()))
}

pub fn root_name(xml: &str) -> Option<String> {
    let mut rest = xml.trim_start();
    if rest.starts_with("<?") {
        let end = rest.find("?>")?;
        rest = rest.get(end + 2..)?.trim_start();
    }
    let open = rest.strip_prefix('<')?;
    let name_end = open.find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')?;
    Some(open.get(..name_end)?.to_owned())
}

fn count_open_tags(xml: &str, name: &str) -> usize {
    open_tags(xml, name).len()
}

fn open_tags(xml: &str, name: &str) -> Vec<String> {
    let needle = format!("<{name}");
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(pos_rel) = xml[search..].find(&needle) {
        let pos = search + pos_rel;
        let next = xml.as_bytes().get(pos + needle.len()).copied();
        if matches!(
            next,
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/')
        ) {
            if let Some(end_rel) = xml[pos..].find('>') {
                out.push(xml[pos..=pos + end_rel].to_owned());
                search = pos + end_rel + 1;
                continue;
            }
        }
        search = pos + needle.len();
    }
    out
}

fn attr_value_in_first_tag(xml: &str, tag: &str, attr: &str) -> Option<String> {
    open_tags(xml, tag)
        .into_iter()
        .find_map(|open| attr_value(&open, attr))
}

fn element_body(xml: &str, tag: &str, name_attr: &str) -> Option<String> {
    let needle = format!("<{tag}");
    let mut search = 0usize;
    while let Some(pos_rel) = xml[search..].find(&needle) {
        let pos = search + pos_rel;
        let open_end_rel = xml[pos..].find('>')?;
        let open_end = pos + open_end_rel;
        let open = &xml[pos..=open_end];
        if attr_value(open, "name").as_deref() == Some(name_attr) {
            let close = format!("</{tag}>");
            let close_rel = xml[open_end + 1..].find(&close)?;
            return Some(xml[open_end + 1..open_end + 1 + close_rel].to_owned());
        }
        search = open_end + 1;
    }
    None
}

fn attr_value(open: &str, key: &str) -> Option<String> {
    let bytes = open.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        while i < bytes.len() && !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric()
                || bytes[i] == b'_'
                || bytes[i] == b'-'
                || bytes[i] == b'.'
                || bytes[i] == b':')
        {
            i += 1;
        }
        let found = open.get(key_start..i)?.trim();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || (bytes[i] != b'"' && bytes[i] != b'\'') {
            continue;
        }
        let quote = bytes[i];
        i += 1;
        let value_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        let value = open.get(value_start..i)?.to_owned();
        if found == key {
            return Some(xml_unescape(&value));
        }
        i = i.saturating_add(1);
    }
    None
}

#[cfg(test)]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_xml() -> &'static str {
        r#"<?xml version="1.0" encoding="utf-8" standalone="yes"?>
<NematMaterialLibrary schema="newengine.nemat.xmltype.v1">
  <Entries>
    <Material name="door" shader="pbr.default" domain="surface" shading_model="pbr_metallic_roughness">
      <Surface blend="opaque" two_sided="false" />
      <Textures><Texture slot="base_color" ref="textures/garage.ytd@door_bc" required="true" /></Textures>
      <Params><Param name="roughness" type="float" value="0.72" /></Params>
    </Material>
  </Entries>
</NematMaterialLibrary>"#
    }

    #[test]
    fn xmltype_accepts_ytd_entry_texture_ref() {
        assert!(parse_material_xml(sample_xml()).is_ok());
    }

    #[test]
    fn xmltype_rejects_raw_texture_ref() {
        let xml = sample_xml().replace("textures/garage.ytd@door_bc", "textures/door.png");
        assert!(parse_material_xml(&xml).is_err());
    }

    #[test]
    fn draft_is_xmltype() {
        let cfg = CommonArgs {
            material: Some("door".to_owned()),
            textures: vec!["base_color=textures/garage.ytd@door_bc".to_owned()],
            ..Default::default()
        };
        let xml = xml_from_draft_args(&cfg).unwrap();
        assert!(xml.contains("<NematMaterialLibrary"));
        assert!(xml.contains("<Material name=\"door\""));
    }
}
