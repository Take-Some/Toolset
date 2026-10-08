const PROPERTIES_SCHEMA_ID: &str = "newengine.ytyp.properties.v1";
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRecord {
    pub reference: String,
    pub role: String,
    pub domain: String,
    pub required: bool,
}

pub fn entry_kind(xml: &str) -> String {
    root_attr(xml, "entry_kind")
        .or_else(|| root_attr(xml, "entryKind"))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "archetype_definition".to_owned())
}

pub fn dependency_records(xml: &str) -> Vec<DependencyRecord> {
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(pos_rel) = xml[search..].find('<') {
        let pos = search + pos_rel;
        let Some(open_end_rel) = xml[pos..].find('>') else {
            break;
        };
        let open_end = pos + open_end_rel;
        let open = &xml[pos..=open_end];
        let tag = open
            .trim_start_matches('<')
            .trim_start()
            .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
            .next()
            .unwrap_or_default();
        if tag == "Dependency" {
            let reference = attr_value(open, "reference")
                .or_else(|| attr_value(open, "ref"))
                .unwrap_or_default();
            if !reference.trim().is_empty() {
                let role = attr_value(open, "role").unwrap_or_else(|| "dependency".to_owned());
                let domain = attr_value(open, "domain").unwrap_or_default();
                let required = attr_value(open, "required")
                    .map(|value| {
                        !matches!(
                            value.trim().to_ascii_lowercase().as_str(),
                            "false" | "0" | "no"
                        )
                    })
                    .unwrap_or(true);
                out.push(DependencyRecord {
                    reference,
                    role,
                    domain,
                    required,
                });
            }
        }
        search = open_end + 1;
    }
    out.sort_by(|a, b| {
        (&a.reference, &a.role, &a.domain, a.required).cmp(&(
            &b.reference,
            &b.role,
            &b.domain,
            b.required,
        ))
    });
    out.dedup();
    out
}

pub fn validate_metadata_xml(xml: &str, source_ref: &str) -> Result<Vec<String>, String> {
    let root =
        root_name(xml).ok_or_else(|| format!("{source_ref}: XML document has no root element"))?;
    if root != "YtypProperties" && root != "AssetProperties" {
        return Err(format!(
            "{source_ref}: .ytyp source root must be <YtypProperties> or <AssetProperties>; actual='{root}'"
        ));
    }
    let schema = root_attr(xml, "schema").unwrap_or_default();
    if schema != PROPERTIES_SCHEMA_ID {
        return Err(format!(
            "{source_ref}: .ytyp properties schema must be {PROPERTIES_SCHEMA_ID}; actual='{schema}'"
        ));
    }
    let mut warnings = Vec::new();
    if entry_names(xml).is_empty() {
        warnings.push(
            "YtypProperties has no name/id; runtime identity will fall back to root name"
                .to_owned(),
        );
    }
    Ok(warnings)
}

pub fn manifest_json_for_metadata(xml: &str, logical_path: &str) -> Result<Value, String> {
    validate_metadata_xml(xml, logical_path)?;
    let source = normalize_logical_path(logical_path);
    let name = entry_names(xml)
        .first()
        .cloned()
        .unwrap_or_else(|| "properties".to_owned());
    Ok(json!({
        "schema": "asset.inspect.ytyp.properties.v1",
        "source": source,
        "asset_kind": "asset_properties",
        "container": "newengine.properties.xml.ytyp",
        "semantic_gateway": "engine.assets.definitions",
        "runtime_policy": "Y-Type Properties; one .ytyp file explains one model/asset/type before domain extraction",
        "entry": {
            "name": name,
            "entry_ref": source,
            "asset_kind": "asset_properties",
            "route": {
                "gateway": "engine.assets.definitions",
                "method": "definitions.properties_json_v1",
                "semantic_owner": "engine.assets.definitions"
            }
        },
        "dependencies": dependencies(xml),
        "summary": summary_json(xml),
    }))
}

pub fn metadata_projection_json(xml: &str, logical_path: &str) -> Result<Value, String> {
    validate_metadata_xml(xml, logical_path)?;
    Ok(json!({
        "schema": "newengine.ytyp.properties.projection.v1",
        "note": "diagnostic projection; authoritative interpretation belongs to engine.assets.definitions and consuming model/render domains",
        "document_ref": normalize_logical_path(logical_path),
        "root": root_name(xml).unwrap_or_else(|| "<unknown>".to_owned()),
        "entries": entry_names(xml),
        "dependencies": dependencies(xml),
        "summary": summary_json(xml),
    }))
}

pub fn summary_json(xml: &str) -> Value {
    let entries = entry_names(xml);
    let deps = dependencies(xml);
    json!({
        "root": root_name(xml).unwrap_or_else(|| "<unknown>".to_owned()),
        "entries": entries,
        "entry_count": entries.len(),
        "dependencies": deps,
        "dependency_count": deps.len(),
        "metadata_node_count": count_any_open_tags(xml),
        "profile_hints": profile_hints(xml),
        "render_role": render_role(xml),
    })
}

pub fn entry_names(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(attr_values(xml, "YtypProperties", "name"));
    out.extend(attr_values(xml, "YtypProperties", "id"));
    out.extend(attr_values(xml, "AssetProperties", "name"));
    out.extend(attr_values(xml, "AssetProperties", "id"));
    out.retain(|value| !value.trim().is_empty());
    out.sort();
    out.dedup();
    out
}

pub fn dependencies(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for attr in [
        "ref",
        "reference",
        "asset",
        "asset_ref",
        "assetRef",
        "source",
        "target",
        "material",
        "model",
        "drawable",
        "texture",
        "dictionary",
        "document",
        "metadata",
    ] {
        out.extend(all_attr_values(xml, attr));
    }
    out.retain(|value| looks_like_asset_ref(value));
    out.sort();
    out.dedup();
    out
}

pub fn normalize_logical_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

fn profile_hints(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    out.push("profile.ytyp.properties".to_owned());
    if xml.contains("mesh.role") {
        out.push("hint.render_role".to_owned());
    }
    if xml.contains("material") || xml.contains("Material") {
        out.push("hint.material_refs".to_owned());
    }
    if xml.contains("texture") || xml.contains("Texture") {
        out.push("hint.texture_refs".to_owned());
    }
    if xml.contains("drawable") || xml.contains(".ydd") {
        out.push("hint.drawable_ref".to_owned());
    }
    out.sort();
    out.dedup();
    out
}

fn render_role(xml: &str) -> Option<String> {
    let mut search = 0usize;
    while let Some(pos_rel) = xml[search..].find("<Value") {
        let pos = search + pos_rel;
        let Some(open_end_rel) = xml[pos..].find('>') else {
            break;
        };
        let open_end = pos + open_end_rel;
        let open = &xml[pos..=open_end];
        if attr_value(open, "key").as_deref() == Some("mesh.role") {
            return attr_value(open, "value");
        }
        search = open_end + 1;
    }
    None
}

fn looks_like_asset_ref(value: &str) -> bool {
    let v = value.trim();
    if v.is_empty() {
        return false;
    }
    v.contains('@')
        || v.contains(".ytd")
        || v.contains(".ydd")
        || v.contains(".ytyp")
        || v.contains(".ymap")
        || v.contains(".ymat")
        || v.contains(".neui")
        || v.contains(".ybn")
        || v.contains(".ycol")
}

fn root_name(xml: &str) -> Option<String> {
    let mut rest = xml.trim_start();
    if rest.starts_with("<?") {
        let end = rest.find("?>")?;
        rest = rest.get(end + 2..)?.trim_start();
    }
    let open = rest.strip_prefix('<')?;
    let name_end = open.find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')?;
    Some(open.get(..name_end)?.to_owned())
}

fn root_attr(xml: &str, key: &str) -> Option<String> {
    let mut rest = xml.trim_start();
    if rest.starts_with("<?") {
        let end = rest.find("?>")?;
        rest = rest.get(end + 2..)?.trim_start();
    }
    let open_end = rest.find('>')?;
    attr_value(&rest[..=open_end], key)
}

fn count_any_open_tags(xml: &str) -> usize {
    xml.as_bytes()
        .windows(1)
        .filter(|w| w[0] == b'<')
        .count()
        .saturating_sub(xml.matches("</").count())
}

fn attr_values(xml: &str, tag: &str, attr: &str) -> Vec<String> {
    let needle = format!("<{tag}");
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(pos_rel) = xml[search..].find(&needle) {
        let pos = search + pos_rel;
        let Some(open_end_rel) = xml[pos..].find('>') else {
            break;
        };
        let open_end = pos + open_end_rel;
        let open = &xml[pos..=open_end];
        if let Some(value) = attr_value(open, attr) {
            out.push(value);
        }
        search = open_end + 1;
    }
    out
}

fn all_attr_values(xml: &str, attr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(pos_rel) = xml[search..].find('<') {
        let pos = search + pos_rel;
        let Some(open_end_rel) = xml[pos..].find('>') else {
            break;
        };
        let open_end = pos + open_end_rel;
        let open = &xml[pos..=open_end];
        if !open.starts_with("</") {
            if let Some(value) = attr_value(open, attr) {
                out.push(value);
            }
        }
        search = open_end + 1;
    }
    out
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
            && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'-' | b'.' | b':'))
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

    #[test]
    fn properties_require_ytyp_root() {
        let xml = r#"<YtypProperties schema="newengine.ytyp.properties.v1" name="foo"><Dependency ref="assets/a.ytd@bar" /></YtypProperties>"#;
        assert!(validate_metadata_xml(xml, "x").is_ok());
        assert_eq!(entry_names(xml), vec!["foo"]);
        assert_eq!(dependencies(xml), vec!["assets/a.ytd@bar"]);
    }

    #[test]
    fn ycd_clip_dependencies_are_preserved() {
        let xml = r#"<YtypProperties schema="newengine.ytyp.properties.v1" name="abby"><Dependencies><Dependency reference="animations/characters/abby/idle.ycd@idle" role="animation/idle" domain="engine.animation" required="true" /></Dependencies></YtypProperties>"#;
        let records = dependency_records(xml);
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].reference,
            "animations/characters/abby/idle.ycd@idle"
        );
        assert_eq!(records[0].domain, "engine.animation");
        assert_eq!(
            dependencies(xml),
            vec!["animations/characters/abby/idle.ycd@idle"]
        );
    }

    #[test]
    fn dependency_records_preserve_role_domain_and_required() {
        let xml = r#"<YtypProperties schema="newengine.ytyp.properties.v1" name="foo"><Dependencies><Dependency reference="models/foo.ydd@foo" role="render/drawable" domain="engine.model" required="true" /><Dependency reference="materials/foo.ymat@foo" role="render/material" domain="engine.materials" required="false" /></Dependencies></YtypProperties>"#;
        let records = dependency_records(xml);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].reference, "materials/foo.ymat@foo");
        assert!(!records[0].required);
        assert_eq!(records[1].role, "render/drawable");
        assert_eq!(records[1].domain, "engine.model");
        assert!(records[1].required);
    }
}
