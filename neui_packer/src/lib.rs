use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
const CONTENT_SCHEMA_VERSION: u16 = 1; // Must match the NEUI format descriptor readable/write schema.
use northstar_nef8::{
    body_slice, encode, parse_header as parse_wire_header, EncodeRequest, Header,
    FULL_HASH_BODY_THRESHOLD, TYPE_NEUI,
};
use serde_json::{json, Value};
use std::io::{Read, Write};

pub const CONTENT_KIND_NEUI: u16 = TYPE_NEUI;

pub fn pack_xmlcentral_to_nef8(
    xmlcentral: &str,
    logical_path: &str,
    _import_settings_seed: &str,
    requested_entry_count: u64,
) -> Result<Vec<u8>, String> {
    validate_xmlcentral(xmlcentral, logical_path)?;
    let body = xmlcentral.as_bytes();
    let compressed = deflate(body)?;
    let entry_count = requested_entry_count
        .max(entry_names(xmlcentral).len() as u64)
        .max(1);
    let body_hash =
        (body.len() >= FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(body).as_bytes());
    encode(EncodeRequest {
        content_kind: CONTENT_KIND_NEUI,
        content_schema_version: CONTENT_SCHEMA_VERSION,
        entry_count: u32::try_from(entry_count)
            .map_err(|_| format!("NEUI entry_count too large: {entry_count}"))?,
        additional_flags: 0,
        min_size_class: 4,
        metadata: &[],
        stored_body: &compressed,
        body_uncompressed_len: body.len() as u64,
        body_hash,
        stable_file_id: None,
        import_settings_hash: None,
    })
}

pub fn decode_nef8_xmlcentral(bytes: &[u8]) -> Result<String, String> {
    let header = parse_header(bytes)?;
    if header.content_kind != CONTENT_KIND_NEUI {
        return Err(format!(
            "NEF8 content_kind={} is not ui_dictionary ({})",
            header.content_kind, CONTENT_KIND_NEUI
        ));
    }
    if !header.is_deflate() {
        return Err(format!(
            "NEF8 .neui requires deflate body flags=0x{:04x} compression={}",
            header.flags, header.compression
        ));
    }
    let inflated = inflate(body_slice(bytes, &header)?)?;
    if header.body_uncompressed_len != 0 && inflated.len() as u64 != header.body_uncompressed_len {
        return Err(format!(
            "NEF8 inflated body size mismatch actual={} expected={}",
            inflated.len(),
            header.body_uncompressed_len
        ));
    }
    if header.has_body_hash() && blake3::hash(&inflated).as_bytes() != &header.body_hash {
        return Err("NEF8 body hash mismatch after inflate".to_owned());
    }
    String::from_utf8(inflated)
        .map_err(|error| format!(".neui XMLcentral body is not UTF-8: {error}"))
}

pub fn inspect_nef8_json(bytes: &[u8]) -> Result<Value, String> {
    let header = parse_header(bytes)?;
    let xmlcentral = if header.content_kind == CONTENT_KIND_NEUI {
        Some(decode_nef8_xmlcentral(bytes)?)
    } else {
        None
    };
    let summary = xmlcentral
        .as_deref()
        .map(xmlcentral_summary_json)
        .unwrap_or_else(|| json!({}));
    Ok(json!({
        "schema": "northstar.neui.inspect.v1",
        "ok": header.content_kind == CONTENT_KIND_NEUI,
        "header": {
            "magic": "NEF8",
            "version": header.version,
            "size_class": header.size_class,
            "header_len": header.header_len,
            "content_kind": header.content_kind,
            "content_kind_label": if header.content_kind == CONTENT_KIND_NEUI { "ui_dictionary" } else { "non_ui_dictionary" },
            "flags": header.flags,
            "compression": header.compression,
            "entry_count": header.entry_count,
            "body_offset": header.body_offset,
            "body_len": header.body_len,
            "body_uncompressed_len": header.body_uncompressed_len,
            "stable_file_id": format!("{:016x}", header.stable_file_id),
        },
        "xmlcentral": summary,
    }))
}

pub fn manifest_json_for_xmlcentral(xmlcentral: &str, logical_path: &str) -> Result<Value, String> {
    validate_xmlcentral(xmlcentral, logical_path)?;
    let entries = entry_names(xmlcentral);
    let deps = dependencies(xmlcentral);
    Ok(json!({
        "schema": "asset.inspect.neui.v1",
        "source": normalize_logical_path(logical_path),
        "asset_kind": "ui_dictionary",
        "container": "newengine.listfile.nef8.neui",
        "semantic_gateway": "engine.assets.ui",
        "runtime_gateway": "engine.ui",
        "entries": entries.iter().map(|entry| json!({
            "name": entry,
            "entry_ref": format!("{}@{}", normalize_logical_path(logical_path), entry),
            "asset_kind": "ui_surface_or_library",
            "route": {
                "gateway": "engine.assets.ui",
                "method": "assets.ui.compile_document_v1",
                "semantic_owner": "ui_dictionary"
            }
        })).collect::<Vec<_>>(),
        "dependencies": deps,
        "ui_resource_dependencies": {
            "textures": texture_refs(xmlcentral),
            "fonts": font_refs(xmlcentral)
        },
        "summary": xmlcentral_summary_json(xmlcentral),
    }))
}

pub fn compiled_document_projection_json(
    xmlcentral: &str,
    logical_path: &str,
) -> Result<Value, String> {
    validate_xmlcentral(xmlcentral, logical_path)?;
    let surface_id = first_attr_value(xmlcentral, "Surface", "name")
        .unwrap_or_else(|| "engine.ui.surface".to_owned());
    let root_id =
        first_attr_value(xmlcentral, "Surface", "root").unwrap_or_else(|| "layout.main".to_owned());
    let theme_ref = first_attr_value(xmlcentral, "Surface", "theme");
    Ok(json!({
        "schema": "newengine.ui.compiled_document.projection.v1",
        "note": "diagnostic projection; authoritative runtime compilation is owned by engine.assets.ui",
        "document_ref": normalize_logical_path(logical_path),
        "surface_id": surface_id,
        "root_id": root_id,
        "theme_ref": theme_ref,
        "dependencies": dependencies(xmlcentral),
        "binding_plan": binding_plan_projection_json(xmlcentral),
        "source": {
            "kind": "asset",
            "document_ref": normalize_logical_path(logical_path)
        },
        "xmlcentral_summary": xmlcentral_summary_json(xmlcentral),
    }))
}

pub fn binding_plan_projection_json(xmlcentral: &str) -> Value {
    let binding_count = count_open_tags(xmlcentral, "Binding");
    let event_count = count_open_tags(xmlcentral, "Event");
    json!({
        "schema": "newengine.ui.binding_plan.projection.v1",
        "source": "XMLcentral",
        "binding_count": binding_count,
        "event_count": event_count,
        "policy": "bindings/events are authored in XML and compiled by engine.assets.ui into UiBindingPlan/UiActionEdge DTOs"
    })
}

pub fn xmlcentral_summary_json(xmlcentral: &str) -> Value {
    json!({
        "root": root_name(xmlcentral).unwrap_or_else(|| "<unknown>".to_owned()),
        "entries": entry_names(xmlcentral),
        "dependencies": dependencies(xmlcentral),
        "ui_resource_dependencies": {
            "textures": texture_refs(xmlcentral),
            "fonts": font_refs(xmlcentral)
        },
        "surface_count": count_open_tags(xmlcentral, "Surface"),
        "layout_count": count_open_tags(xmlcentral, "Layout"),
        "theme_count": count_open_tags(xmlcentral, "Theme"),
        "component_template_count": count_open_tags(xmlcentral, "ComponentTemplate"),
        "binding_count": count_open_tags(xmlcentral, "Binding"),
        "event_count": count_open_tags(xmlcentral, "Event"),
    })
}

pub fn validate_xmlcentral(xmlcentral: &str, source_ref: &str) -> Result<Vec<String>, String> {
    let root = root_name(xmlcentral)
        .ok_or_else(|| format!("{source_ref}: XML document has no root element"))?;
    let mut warnings = Vec::new();
    match root.as_str() {
        "NeUiDictionary" => {
            if count_open_tags(xmlcentral, "Surface") == 0 {
                warnings.push("NeUiDictionary has no <Surface>; it can be packed but engine.assets.ui cannot mount it as a live surface until one is authored".to_owned());
            }
        }
        "NeUiThemeLibrary" => {
            if count_open_tags(xmlcentral, "Theme") == 0 {
                warnings.push("NeUiThemeLibrary has no <Theme>; it can be packed as XML data but exposes no theme entries".to_owned());
            }
        }
        other => warnings.push(format!(
            "generic XML root '{other}' accepted for NEF8/ListFile storage; runtime UI compilation expects NeUiDictionary or NeUiThemeLibrary"
        )),
    }
    if entry_names(xmlcentral).is_empty() {
        warnings.push(
            "no <Entry> tags found; runtime entry will fall back to surface/theme/default name"
                .to_owned(),
        );
    }
    validate_ui_resource_refs(xmlcentral, source_ref)?;
    Ok(warnings)
}

pub fn entry_names(xmlcentral: &str) -> Vec<String> {
    let mut out = attr_values(xmlcentral, "Entry", "name");
    if out.is_empty() {
        out.extend(attr_values(xmlcentral, "Surface", "name"));
    }
    if out.is_empty() {
        out.extend(attr_values(xmlcentral, "Theme", "name"));
    }
    if out.is_empty() {
        out.push("surface".to_owned());
    }
    out.sort();
    out.dedup();
    out
}

pub fn dependencies(xmlcentral: &str) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(attr_values(xmlcentral, "ThemeRef", "ref"));
    out.extend(attr_values(xmlcentral, "ComponentRef", "ref"));
    out.extend(attr_values(xmlcentral, "Import", "ref"));
    out.extend(attr_values(xmlcentral, "Surface", "theme"));
    out.extend(font_refs(xmlcentral));
    out.extend(texture_refs(xmlcentral));
    out.retain(|value| !value.trim().is_empty());
    out.sort();
    out.dedup();
    out
}

pub fn font_refs(xmlcentral: &str) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(attr_values(xmlcentral, "FontRef", "ref"));
    out.extend(attr_values(xmlcentral, "Font", "ref"));
    out.extend(attr_values(xmlcentral, "TextStyle", "font"));
    out.extend(attr_values(xmlcentral, "Text", "font"));
    out.extend(attr_values(xmlcentral, "Label", "font"));
    out.extend(attr_values_by_attr_name(xmlcentral, "fontRef"));
    out.retain(|value| is_asset_ref(value) && value.to_ascii_lowercase().contains(".yfd@"));
    out.sort();
    out.dedup();
    out
}

pub fn texture_refs(xmlcentral: &str) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(attr_values(xmlcentral, "TextureRef", "ref"));
    out.extend(attr_values(xmlcentral, "Image", "texture"));
    out.extend(attr_values(xmlcentral, "Image", "src"));
    out.extend(attr_values(xmlcentral, "Icon", "texture"));
    out.extend(attr_values(xmlcentral, "Icon", "src"));
    out.extend(attr_values(xmlcentral, "Background", "texture"));
    out.extend(attr_values(xmlcentral, "Brush", "texture"));
    out.extend(attr_values_by_attr_name(xmlcentral, "textureRef"));
    out.extend(attr_values_by_attr_name(xmlcentral, "imageRef"));
    out.retain(|value| is_asset_ref(value) && value.to_ascii_lowercase().contains(".ytd@"));
    out.sort();
    out.dedup();
    out
}

pub fn normalize_logical_path(value: &str) -> String {
    let clean = value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned();
    if clean.starts_with("assets/") || clean.is_empty() {
        clean
    } else {
        format!("assets/{clean}")
    }
}

fn parse_header(bytes: &[u8]) -> Result<Header, String> {
    parse_wire_header(bytes)
}

fn deflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())
}

fn inflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = DeflateDecoder::new(bytes);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| format!("deflate decode failed: {e}"))?;
    Ok(out)
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

fn count_open_tags(xml: &str, name: &str) -> usize {
    let needle = format!("<{name}");
    let mut count = 0usize;
    let mut search = 0usize;
    while let Some(pos_rel) = xml[search..].find(&needle) {
        let pos = search + pos_rel;
        let next = xml.as_bytes().get(pos + needle.len()).copied();
        if matches!(
            next,
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/')
        ) {
            count += 1;
        }
        search = pos + needle.len();
    }
    count
}

fn first_attr_value(xml: &str, tag: &str, attr: &str) -> Option<String> {
    attr_values(xml, tag, attr).into_iter().next()
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

fn attr_values_by_attr_name(xml: &str, attr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(pos_rel) = xml[search..].find('<') {
        let pos = search + pos_rel;
        let Some(open_end_rel) = xml[pos..].find('>') else {
            break;
        };
        let open_end = pos + open_end_rel;
        let open = &xml[pos..=open_end];
        if !open.starts_with("</") && !open.starts_with("<!--") {
            if let Some(value) = attr_value(open, attr) {
                out.push(value);
            }
        }
        search = open_end + 1;
    }
    out
}

fn is_asset_ref(value: &str) -> bool {
    let clean = value.trim().replace('\\', "/");
    clean.contains('@')
        && (clean.starts_with("assets/")
            || clean.starts_with("./assets/")
            || clean.starts_with("/assets/"))
}

fn validate_ui_resource_refs(xmlcentral: &str, source_ref: &str) -> Result<(), String> {
    let mut refs = Vec::new();
    refs.extend(attr_values(xmlcentral, "FontRef", "ref"));
    refs.extend(attr_values(xmlcentral, "Font", "ref"));
    refs.extend(attr_values(xmlcentral, "TextStyle", "font"));
    refs.extend(attr_values(xmlcentral, "Text", "font"));
    refs.extend(attr_values(xmlcentral, "Label", "font"));
    refs.extend(attr_values_by_attr_name(xmlcentral, "fontRef"));
    refs.extend(attr_values(xmlcentral, "TextureRef", "ref"));
    refs.extend(attr_values(xmlcentral, "Image", "texture"));
    refs.extend(attr_values(xmlcentral, "Image", "src"));
    refs.extend(attr_values(xmlcentral, "Icon", "texture"));
    refs.extend(attr_values(xmlcentral, "Icon", "src"));
    refs.extend(attr_values(xmlcentral, "Background", "texture"));
    refs.extend(attr_values(xmlcentral, "Brush", "texture"));
    refs.extend(attr_values_by_attr_name(xmlcentral, "textureRef"));
    refs.extend(attr_values_by_attr_name(xmlcentral, "imageRef"));

    for value in refs {
        let lowered = value.to_ascii_lowercase();
        if lowered.contains(".yft") {
            return Err(format!(
                "{source_ref}: UI font ref '{value}' uses unsupported .yft; fonts must be .yfd@entry"
            ));
        }
        if lowered.contains(".yfd") && !lowered.contains(".yfd@") {
            return Err(format!(
                "{source_ref}: UI font ref '{value}' must select a YFD entry with .yfd@entry"
            ));
        }
        if lowered.contains(".ytd") && !lowered.contains(".ytd@") {
            return Err(format!(
                "{source_ref}: UI texture ref '{value}' must select a YTD entry with .ytd@entry"
            ));
        }
    }
    Ok(())
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
<NeUiDictionary schema="newengine.neui.xmlcentral.v1">
  <Entries><Entry name="surface" kind="ui_surface" /></Entries>
  <Surface name="engine.ui.test" root="layout.main" theme="assets/ui/themes/northstar_editor.neui@editor_light" />
  <Resources>
    <TextureRef name="logo" ref="assets/loading/loading_ui.ytd@newengine_logo" />
    <FontRef name="body" ref="assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold" />
  </Resources>
  <Layout name="layout.main"><Panel id="root"><Image texture="assets/loading/loading_ui.ytd@newengine_logo" /><Text font="assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold" value="OK" /></Panel></Layout>
</NeUiDictionary>"#
    }

    #[test]
    fn neui_rejects_bad_magic() {
        let err = inspect_nef8_json(b"BAD!").unwrap_err();
        assert!(err.contains("prologue") || err.contains("magic"));
    }

    #[test]
    fn neui_rejects_wrong_content_kind() {
        let mut bytes =
            pack_xmlcentral_to_nef8(sample_xml(), "assets/ui/test.neui", "test", 1).unwrap();
        bytes[6] = 7;
        let err = decode_nef8_xmlcentral(&bytes).unwrap_err();
        assert!(err.contains("not ui_dictionary"));
    }

    #[test]
    fn neui_compile_then_inspect_roundtrip() {
        let bytes =
            pack_xmlcentral_to_nef8(sample_xml(), "assets/ui/test.neui", "test", 1).unwrap();
        let header = parse_header(&bytes).unwrap();
        assert_eq!(header.version, northstar_nef8::VERSION);
        assert!(matches!(header.header_len, 32 | 64));
        let xml = decode_nef8_xmlcentral(&bytes).unwrap();
        assert!(xml.contains("<NeUiDictionary"));
        let inspect = inspect_nef8_json(&bytes).unwrap();
        assert_eq!(inspect["ok"], true);
        assert_eq!(inspect["xmlcentral"]["surface_count"], 1);
    }

    #[test]
    fn neui_manifest_contains_file_entry_refs() {
        let manifest = manifest_json_for_xmlcentral(sample_xml(), "assets/ui/test.neui").unwrap();
        assert_eq!(
            manifest["entries"][0]["entry_ref"],
            "assets/ui/test.neui@surface"
        );
        assert_eq!(manifest["semantic_gateway"], "engine.assets.ui");
    }

    #[test]
    fn neui_dependencies_include_ytd_textures_and_yfd_fonts() {
        let deps = dependencies(sample_xml());
        assert!(deps.contains(&"assets/loading/loading_ui.ytd@newengine_logo".to_owned()));
        assert!(deps.contains(&"assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold".to_owned()));
        let manifest = manifest_json_for_xmlcentral(sample_xml(), "assets/ui/test.neui").unwrap();
        assert_eq!(
            manifest["ui_resource_dependencies"]["textures"][0],
            "assets/loading/loading_ui.ytd@newengine_logo"
        );
        assert_eq!(
            manifest["ui_resource_dependencies"]["fonts"][0],
            "assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold"
        );
    }

    #[test]
    fn neui_rejects_unsupported_yft_font_refs() {
        let xml = r#"<NeUiDictionary schema="newengine.neui.xmlcentral.v1"><Surface name="s" root="r" /><Text font="assets/ui/fonts/editor.yft@regular" /></NeUiDictionary>"#;
        let err = validate_xmlcentral(xml, "bad.neui.xml").unwrap_err();
        assert!(err.contains("unsupported .yft"));
    }

    #[test]
    fn neui_rejects_texture_refs_without_ytd_entry() {
        let xml = r#"<NeUiDictionary schema="newengine.neui.xmlcentral.v1"><Surface name="s" root="r" /><Image texture="assets/ui/textures/editor.ytd" /></NeUiDictionary>"#;
        let err = validate_xmlcentral(xml, "bad.neui.xml").unwrap_err();
        assert!(err.contains("must select a YTD entry"));
    }
}
