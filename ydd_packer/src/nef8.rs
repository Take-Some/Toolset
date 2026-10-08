use std::collections::HashSet;

use std::io::{Read, Write};

use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use northstar_nef8::{
    body_slice, encode, parse_header as parse_wire_header, EncodeRequest, Header,
    FULL_HASH_BODY_THRESHOLD, TYPE_YDD,
};

use serde_json::json;

use crate::drawable::{
    stable_hash64, DrawableDictionary, DrawableModel, SkinVertex, Vertex,
    CONTENT_KIND_YDD_DRAWABLE_DICTIONARY,
};

use newengine_asset_format_nef8::ydd_binary::{
    decode_ydd_binary_body, encode_ydd_binary_body, YddBinaryDocument, YddBinaryEntry,
    YddBinaryMesh, YddBinarySkinVertex, YddBinaryVertex, YDD_BINARY_ENCODING,
    YDD_BINARY_SCHEMA_VERSION as BODY_SCHEMA_VERSION,
    YDD_BINARY_SCHEMA_VERSION_V2 as BODY_SCHEMA_VERSION_V2,
    YDD_BINARY_SCHEMA_VERSION_V3 as BODY_SCHEMA_VERSION_V3,
    YDD_BINARY_SCHEMA_VERSION_V4 as BODY_SCHEMA_VERSION_V4,
};

const BODY_HEADER_LEN: usize = 40;

const ENTRY_RECORD_LEN: usize = 80;
pub type Nef8Header = Header;

#[derive(Debug, Clone)]

pub struct ResidentEntryInfo {
    pub name: String,

    pub selector: String,

    pub mesh_count: u32,

    pub vertex_count: u32,

    pub index_count: u32,

    pub material_count: u32,

    pub payload_len: u64,
}

#[derive(Debug, Clone)]

pub struct ParsedDrawableDictionary {
    pub header: Nef8Header,

    pub entries: Vec<ResidentEntryInfo>,
}

pub fn pack_ydd(dict: &DrawableDictionary, _logical_path: &str) -> Result<Vec<u8>, String> {
    validate_dictionary(dict)?;
    debug_assert_eq!(CONTENT_KIND_YDD_DRAWABLE_DICTIONARY, TYPE_YDD);
    // First-party producers always emit the canonical YDD body schema.
    // V2-V4 remain reader-only migration inputs.
    let body_schema_version = BODY_SCHEMA_VERSION;
    let body = encode_ydd_binary_body(&to_binary_document(dict)?)?;
    let compressed = deflate(&body)?;
    let body_hash =
        (body.len() >= FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(&body).as_bytes());
    encode(EncodeRequest {
        content_kind: CONTENT_KIND_YDD_DRAWABLE_DICTIONARY,
        content_schema_version: body_schema_version as u16,
        entry_count: dict.models.len().min(u32::MAX as usize) as u32,
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

pub fn decode_dictionary(bytes: &[u8], file_name: &str) -> Result<DrawableDictionary, String> {
    let header = parse_header(bytes)?;
    if header.content_kind != CONTENT_KIND_YDD_DRAWABLE_DICTIONARY {
        return Err(format!(
            "NEF8 content_kind={} is not ydd drawable_dictionary ({})",
            header.content_kind, CONTENT_KIND_YDD_DRAWABLE_DICTIONARY
        ));
    }
    let body = decode_body(bytes, &header)?;
    decode_binary_body_dictionary(&body, file_name)
}

pub fn parse_ydd(bytes: &[u8], file_name: &str) -> Result<ParsedDrawableDictionary, String> {
    let header = parse_header(bytes)?;

    if header.content_kind != CONTENT_KIND_YDD_DRAWABLE_DICTIONARY {
        return Err(format!(
            "NEF8 content_kind={} is not ydd drawable_dictionary ({})",
            header.content_kind, CONTENT_KIND_YDD_DRAWABLE_DICTIONARY
        ));
    }

    let body = decode_body(bytes, &header)?;

    let entries = parse_body_index(&body, file_name)?;

    Ok(ParsedDrawableDictionary { header, entries })
}

pub fn inspect_json(bytes: &[u8], file_name: &str) -> Result<serde_json::Value, String> {
    let parsed = parse_ydd(bytes, file_name)?;

    let entries = parsed
        .entries
        .iter()
        .map(|e| {
            json!({

                "name": e.name,

                "selector": e.selector,

                "mesh_count": e.mesh_count,

                "vertex_count": e.vertex_count,

                "index_count": e.index_count,

                "material_count": e.material_count,

                "payload_len": e.payload_len,

            })
        })
        .collect::<Vec<_>>();

    Ok(json!({

        "schema": "northstar.ydd.inspect.v1",

        "ok": true,

        "container": "NEF8 ListFile",

        "content_kind": "drawable_dictionary",

        "resident": true,

        "file": normalize_logical_path(file_name),

        "header": {

            "magic": "NEF8",

            "version": parsed.header.version,

            "size_class": parsed.header.size_class,

            "header_len": parsed.header.header_len,

            "content_kind": parsed.header.content_kind,

            "compression": parsed.header.compression,

            "entry_count": parsed.header.entry_count,

            "body_offset": parsed.header.body_offset,

            "body_len": parsed.header.body_len,

            "body_uncompressed_len": parsed.header.body_uncompressed_len,

            "stable_file_id": format!("{:016x}", parsed.header.stable_file_id),

        },

        "body_encoding": YDD_BINARY_ENCODING,
        "drawable_dictionary": { "model_count": parsed.entries.len(), "entries": entries }

    }))
}

pub fn decode_body(bytes: &[u8], header: &Nef8Header) -> Result<Vec<u8>, String> {
    if !header.is_deflate() {
        return Err(format!(
            "YDD NEF8 body must be deflate flags=0x{:04x} compression={}",
            header.flags, header.compression
        ));
    }
    let inflated = inflate(body_slice(bytes, header)?)?;
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
    Ok(inflated)
}

#[inline]
pub fn parse_header(bytes: &[u8]) -> Result<Nef8Header, String> {
    parse_wire_header(bytes)
}

pub fn set_properties_ref(
    bytes: &[u8],
    logical_path: &str,
    entry_name: Option<&str>,
    properties_ref: &str,
) -> Result<Vec<u8>, String> {
    let header = parse_header(bytes)?;
    if header.content_kind != CONTENT_KIND_YDD_DRAWABLE_DICTIONARY {
        return Err(format!(
            "NEF8 content_kind={} is not ydd drawable_dictionary ({})",
            header.content_kind, CONTENT_KIND_YDD_DRAWABLE_DICTIONARY
        ));
    }
    let body = decode_body(bytes, &header)?;
    let mut dictionary = decode_binary_body_dictionary(&body, logical_path)?;
    let properties_ref = normalize_logical_path(properties_ref);
    if properties_ref.trim().is_empty() {
        return Err("YDD properties_ref must not be empty".to_owned());
    }

    let mut updated = 0_usize;
    for model in &mut dictionary.models {
        let selected = entry_name
            .map(|name| model.name.eq_ignore_ascii_case(name))
            .unwrap_or(true);
        if selected {
            model.properties_ref = Some(properties_ref.clone());
            updated += 1;
        }
    }
    if updated == 0 {
        return Err(format!(
            "YDD entry '{}' not found",
            entry_name.unwrap_or_default()
        ));
    }
    pack_ydd(&dictionary, logical_path)
}

pub fn validate_dictionary(dict: &DrawableDictionary) -> Result<(), String> {
    if dict.models.is_empty() {
        return Err("YDD drawable dictionary must contain at least one resident model".to_owned());
    }

    let mut names = HashSet::new();

    let mut hashes = HashSet::new();

    for model in &dict.models {
        if model.name.trim().is_empty() {
            return Err("YDD model entry has empty name".to_owned());
        }

        let lower = model.name.to_ascii_lowercase();

        if !names.insert(lower.clone()) {
            return Err(format!("duplicate YDD model name '{}'", model.name));
        }

        let hash = stable_hash64(&lower);

        if !hashes.insert(hash) {
            return Err(format!("duplicate YDD model hash for '{}'", model.name));
        }

        if model.meshes.is_empty() {
            return Err(format!("YDD model '{}' has no meshes", model.name));
        }
        let has_skin = model.meshes.iter().any(|mesh| mesh.skin.is_some());
        if has_skin && model.skin_source_to_model.is_none() {
            return Err(format!(
                "YDD model '{}' contains skinned meshes but no source-to-model transform",
                model.name
            ));
        }
        if let Some(matrix) = model.skin_source_to_model {
            if matrix.iter().any(|value| !value.is_finite()) {
                return Err(format!(
                    "YDD model '{}' source-to-model transform contains NaN/Inf",
                    model.name
                ));
            }
        }

        for mesh in &model.meshes {
            if mesh.vertices.is_empty() {
                return Err(format!(
                    "YDD model '{}' mesh '{}' has no vertices",
                    model.name, mesh.name
                ));
            }

            if mesh.indices.len() % 3 != 0 {
                return Err(format!(
                    "YDD model '{}' mesh '{}' index_count is not triangulated",
                    model.name, mesh.name
                ));
            }

            if mesh.material_ref.is_some() && mesh.inline_material.is_some() {
                return Err(format!(
                    "YDD model '{}' mesh '{}' cannot contain both material_ref and inline_material",
                    model.name, mesh.name
                ));
            }
            if let Some(material) = &mesh.material_ref {
                crate::drawable::validate_material_ref(material)?;
            }
            if let Some(material) = &mesh.inline_material {
                crate::drawable::validate_inline_material(material)?;
            }

            for v in &mesh.vertices {
                for f in v.position.iter().chain(v.normal.iter()).chain(v.uv0.iter()) {
                    if !f.is_finite() {
                        return Err(format!(
                            "YDD model '{}' contains NaN/Inf vertex data",
                            model.name
                        ));
                    }
                }
            }
            if let Some(skin) = &mesh.skin {
                if skin.len() != mesh.vertices.len() {
                    return Err(format!(
                        "YDD model '{}' mesh '{}' skin count mismatch vertices={} skin={}",
                        model.name,
                        mesh.name,
                        mesh.vertices.len(),
                        skin.len()
                    ));
                }
                for (vertex_index, skin_vertex) in skin.iter().enumerate() {
                    if skin_vertex
                        .weights
                        .iter()
                        .chain(skin_vertex.weights_extra.iter())
                        .any(|value| !value.is_finite() || *value < 0.0)
                    {
                        return Err(format!(
                            "YDD model '{}' mesh '{}' has invalid skin weights vertex={vertex_index}",
                            model.name, mesh.name
                        ));
                    }
                    let sum = skin_vertex
                        .weights
                        .iter()
                        .chain(skin_vertex.weights_extra.iter())
                        .sum::<f32>();
                    if !sum.is_finite() || (sum - 1.0).abs() > 0.01 {
                        return Err(format!(
                            "YDD model '{}' mesh '{}' skin weights are not normalized vertex={vertex_index} sum={sum}",
                            model.name, mesh.name
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn body_is_binary_ydd(bytes: &[u8]) -> Result<bool, String> {
    let header = parse_header(bytes)?;
    let body = decode_body(bytes, &header)?;
    if body
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'{')
    {
        return Ok(false);
    }
    if body.len() < BODY_HEADER_LEN {
        return Ok(false);
    }
    let version = read_u32(&body, 0)?;
    Ok(matches!(
        version,
        BODY_SCHEMA_VERSION_V2
            | BODY_SCHEMA_VERSION_V3
            | BODY_SCHEMA_VERSION_V4
            | BODY_SCHEMA_VERSION
    ))
}

fn decode_binary_body_dictionary(
    body: &[u8],
    file_name: &str,
) -> Result<DrawableDictionary, String> {
    let document = decode_ydd_binary_body(body)
        .map_err(|error| format!("YDD binary body decode failed: {error}"))?;
    let mut models = Vec::with_capacity(document.entries.len());
    for entry in document.entries {
        let mut meshes = Vec::with_capacity(entry.meshes.len());
        for source_mesh in entry.meshes {
            let vertices = source_mesh
                .vertices
                .into_iter()
                .map(|vertex| Vertex {
                    position: vertex.position,
                    normal: vertex.normal,
                    uv0: vertex.uv0,
                })
                .collect::<Vec<_>>();
            let skin = source_mesh.skin.map(|stream| {
                stream
                    .into_iter()
                    .map(|vertex| SkinVertex {
                        joints: vertex.joints,
                        weights: vertex.weights,
                        joints_extra: vertex.joints_extra,
                        weights_extra: vertex.weights_extra,
                    })
                    .collect::<Vec<_>>()
            });
            meshes.push(crate::drawable::DrawableMesh {
                name: source_mesh.name,
                material_ref: source_mesh.material_ref,
                inline_material: source_mesh.inline_material,
                vertices,
                skin,
                indices: source_mesh.indices,
                bounds: crate::drawable::Bounds3 {
                    min: source_mesh.bounds_min,
                    max: source_mesh.bounds_max,
                },
            });
        }
        let bounds = crate::drawable::Bounds3 {
            min: entry.bounds_min,
            max: entry.bounds_max,
        };
        models.push(DrawableModel {
            name: entry.name,
            source_path: if entry.source_path.trim().is_empty() {
                normalize_logical_path(file_name)
            } else {
                entry.source_path
            },
            properties_ref: entry.properties_ref,
            skin_source_to_model: entry.skin_source_to_model,
            meshes,
            bounds,
        });
    }
    Ok(DrawableDictionary::new(models))
}

fn to_binary_document(dict: &DrawableDictionary) -> Result<YddBinaryDocument, String> {
    let entries = dict
        .models
        .iter()
        .map(|model| {
            let meshes = model
                .meshes
                .iter()
                .map(|mesh| {
                    if mesh.material_ref.is_some() && mesh.inline_material.is_some() {
                        return Err(format!(
                            "YDD model '{}' mesh '{}' cannot contain both external and inline material",
                            model.name, mesh.name
                        ));
                    }
                    let vertices = mesh
                        .vertices
                        .iter()
                        .map(|vertex| YddBinaryVertex {
                            position: vertex.position,
                            normal: vertex.normal,
                            uv0: vertex.uv0,
                        })
                        .collect();
                    let skin = mesh.skin.as_ref().map(|stream| {
                        stream
                            .iter()
                            .map(|vertex| YddBinarySkinVertex {
                                joints: vertex.joints,
                                weights: vertex.weights,
                                joints_extra: vertex.joints_extra,
                                weights_extra: vertex.weights_extra,
                            })
                            .collect()
                    });
                    Ok(YddBinaryMesh {
                        name: mesh.name.clone(),
                        material_ref: mesh.material_ref.clone(),
                        inline_material: mesh.inline_material.clone(),
                        bounds_min: mesh.bounds.min,
                        bounds_max: mesh.bounds.max,
                        vertices,
                        skin,
                        indices: mesh.indices.clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(YddBinaryEntry {
                name: model.name.clone(),
                source_path: model.source_path.clone(),
                properties_ref: model.properties_ref.clone(),
                bounds_min: model.bounds.min,
                bounds_max: model.bounds.max,
                skin_source_to_model: model.skin_source_to_model,
                meshes,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(YddBinaryDocument { entries })
}

fn parse_body_index(body: &[u8], file_name: &str) -> Result<Vec<ResidentEntryInfo>, String> {
    if body.len() < BODY_HEADER_LEN {
        return Err(format!("YDD body too small: {}", body.len()));
    }

    let version = read_u32(body, 0)?;

    if !matches!(
        version,
        BODY_SCHEMA_VERSION_V2
            | BODY_SCHEMA_VERSION_V3
            | BODY_SCHEMA_VERSION_V4
            | BODY_SCHEMA_VERSION
    ) {
        return Err(format!(
            "unsupported YDD body schema version {version}; supported=[{BODY_SCHEMA_VERSION_V2},{BODY_SCHEMA_VERSION_V3},{BODY_SCHEMA_VERSION_V4},{BODY_SCHEMA_VERSION}]"
        ));
    }

    let entry_count = read_u32(body, 4)? as usize;

    let table_offset = read_u64(body, 8)? as usize;

    let string_offset = read_u64(body, 16)? as usize;

    let string_len = read_u64(body, 24)? as usize;

    let payload_offset = read_u64(body, 32)? as usize;

    if table_offset
        .checked_add(entry_count * ENTRY_RECORD_LEN)
        .ok_or("YDD entry table range overflow")?
        > body.len()
    {
        return Err("YDD entry table outside body".to_owned());
    }

    if string_offset
        .checked_add(string_len)
        .ok_or("YDD string table range overflow")?
        > body.len()
    {
        return Err("YDD string table outside body".to_owned());
    }

    if payload_offset > body.len() {
        return Err("YDD payload offset outside body".to_owned());
    }

    let strings = &body[string_offset..string_offset + string_len];

    let mut entries = Vec::with_capacity(entry_count);

    for i in 0..entry_count {
        let o = table_offset + i * ENTRY_RECORD_LEN;

        let name_offset = read_u32(body, o + 8)?;

        let name = read_string(strings, name_offset)?;

        let payload_start = read_u64(body, o + 64)? as usize;

        let payload_len = read_u64(body, o + 72)? as usize;

        if payload_start < payload_offset {
            return Err(format!(
                "YDD entry '{}' payload starts before payload table",
                name
            ));
        }

        if payload_start
            .checked_add(payload_len)
            .ok_or("YDD entry payload range overflow")?
            > body.len()
        {
            return Err(format!("YDD entry '{}' payload outside body", name));
        }

        entries.push(ResidentEntryInfo {
            selector: format!("{}@{}", normalize_logical_path(file_name), name),

            name,

            mesh_count: read_u32(body, o + 16)?,

            vertex_count: read_u32(body, o + 20)?,

            index_count: read_u32(body, o + 24)?,

            material_count: read_u32(body, o + 28)?,

            payload_len: payload_len as u64,
        });
    }

    Ok(entries)
}

fn read_string(strings: &[u8], offset: u32) -> Result<String, String> {
    let start = offset as usize;
    if start >= strings.len() {
        return Err(format!("YDD string offset {offset} outside table"));
    }
    let len = strings[start..]
        .iter()
        .position(|b| *b == 0)
        .ok_or("YDD string is not nul-terminated")?;
    String::from_utf8(strings[start..start + len].to_vec())
        .map_err(|e| format!("YDD string is not UTF-8: {e}"))
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

pub fn normalize_logical_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let s = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated u32 at {offset}"))?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let s = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| format!("truncated u64 at {offset}"))?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
