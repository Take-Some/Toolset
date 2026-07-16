use std::path::Path;

pub fn validate_source_body(body: &[u8], source: &Path) -> Result<(), String> {
    let text = std::str::from_utf8(body).map_err(|e| {
        format!(
            "ymap body is not UTF-8 source='{}' err='{e}'",
            source.display()
        )
    })?;
    let trimmed = text.trim_start();
    if trimmed.starts_with('<') || trimmed.starts_with("<?xml") {
        let root = body_root_label(body);
        if matches!(root.as_str(), "YmapMapDefinition" | "MapDefinition") {
            return Ok(());
        }
        return Err(format!(
            "unsupported .ymap XML root source='{}' actual='{}' expected='YmapMapDefinition|MapDefinition'",
            source.display(), root
        ));
    }
    let json = serde_json::from_str::<serde_json::Value>(text).map_err(|e| {
        format!(
            "ymap body is neither supported XML nor JSON source='{}' err='{e}'",
            source.display()
        )
    })?;
    let schema = json
        .get("schema")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if schema.starts_with("newengine.map.definition.") {
        Ok(())
    } else {
        Err(format!(
            "unsupported .ymap JSON schema source='{}' actual='{}' expected='newengine.map.definition.*'",
            source.display(), schema
        ))
    }
}

pub fn body_root_label(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let mut s = text.trim_start();
    if let Some(rest) = s.strip_prefix("<?xml") {
        if let Some(end) = rest.find("?>") {
            s = rest[end + 2..].trim_start();
        }
    }
    while let Some(rest) = s.strip_prefix("<!--") {
        if let Some(end) = rest.find("-->") {
            s = rest[end + 3..].trim_start();
        } else {
            break;
        }
    }
    if let Some(rest) = s.strip_prefix('<') {
        let name = rest
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '-' || *ch == ':')
            .collect::<String>();
        if !name.is_empty() {
            return name;
        }
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(schema) = value.get("schema").and_then(|value| value.as_str()) {
            return format!("json:{schema}");
        }
        return "json".to_owned();
    }
    "unknown".to_owned()
}

pub fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
