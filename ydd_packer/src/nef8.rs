use std::collections::HashSet;

use std::io::{Read, Write};

use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use northstar_nef8::{
    body_slice, encode, parse_header as parse_wire_header, EncodeRequest, Header,
    FULL_HASH_BODY_THRESHOLD, TYPE_YDD,
};

use serde_json::json;

use crate::drawable::{
    stable_hash64, DrawableDictionary, DrawableModel, Vertex, CONTENT_KIND_YDD_DRAWABLE_DICTIONARY,
};

const BODY_SCHEMA_VERSION: u32 = 2;

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
    let body = encode_body(dict)?;
    let compressed = deflate(&body)?;
    let body_hash =
        (body.len() >= FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(&body).as_bytes());
    encode(EncodeRequest {
        content_kind: CONTENT_KIND_YDD_DRAWABLE_DICTIONARY,
        content_schema_version: BODY_SCHEMA_VERSION as u16,
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

        "body_encoding": "newengine.ydd.binary_mesh",
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

            if let Some(material) = &mesh.material_ref {
                crate::drawable::validate_material_ref(material)?;
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
    Ok(body.len() >= BODY_HEADER_LEN && read_u32(&body, 0)? == BODY_SCHEMA_VERSION)
}

fn decode_binary_body_dictionary(
    body: &[u8],
    file_name: &str,
) -> Result<DrawableDictionary, String> {
    if body.len() < BODY_HEADER_LEN {
        return Err(format!("YDD binary body too small: {}", body.len()));
    }
    let version = read_u32(body, 0)?;
    if version != BODY_SCHEMA_VERSION {
        return Err(format!("unsupported YDD body schema version {version}"));
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
    let mut models = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let o = table_offset + i * ENTRY_RECORD_LEN;
        let name = read_string(strings, read_u32(body, o + 8)?)?;
        let source_path = read_string(strings, read_u32(body, o + 12)?)
            .unwrap_or_else(|_| normalize_logical_path(file_name));
        let properties_offset = read_u32(body, o + 32)?;
        let properties_ref = if properties_offset == u32::MAX {
            None
        } else {
            Some(read_string(strings, properties_offset)?)
        };
        let payload_start = read_u64(body, o + 64)? as usize;
        let payload_len = read_u64(body, o + 72)? as usize;
        if payload_start < payload_offset {
            return Err(format!(
                "YDD entry '{}' payload starts before payload table",
                name
            ));
        }
        let payload_end = payload_start
            .checked_add(payload_len)
            .ok_or("YDD entry payload range overflow")?;
        if payload_end > body.len() {
            return Err(format!("YDD entry '{}' payload outside body", name));
        }
        let mut cursor = payload_start;
        let mesh_count = read_u32(body, cursor)? as usize;
        cursor += 8;
        let mut meshes = Vec::with_capacity(mesh_count);
        for mesh_index in 0..mesh_count {
            if cursor + 40 > payload_end {
                return Err(format!(
                    "YDD entry '{}' mesh {} header truncated",
                    name, mesh_index
                ));
            }
            let mesh_name_offset = read_u32(body, cursor)?;
            cursor += 4;
            let material_offset = read_u32(body, cursor)?;
            cursor += 4;
            let vertex_count = read_u32(body, cursor)? as usize;
            cursor += 4;
            let index_count = read_u32(body, cursor)? as usize;
            cursor += 4;
            let bounds_min = read_f32_array3(body, cursor)?;
            cursor += 12;
            let bounds_max = read_f32_array3(body, cursor)?;
            cursor += 12;
            let mesh_name = read_string(strings, mesh_name_offset)
                .unwrap_or_else(|_| format!("mesh_{mesh_index}"));
            let material_ref = if material_offset == u32::MAX {
                None
            } else {
                Some(read_string(strings, material_offset)?)
            };
            let vertex_bytes = vertex_count
                .checked_mul(32)
                .ok_or("YDD vertex byte range overflow")?;
            if cursor
                .checked_add(vertex_bytes)
                .ok_or("YDD vertex range overflow")?
                > payload_end
            {
                return Err(format!(
                    "YDD entry '{}' mesh '{}' vertices outside payload",
                    name, mesh_name
                ));
            }
            let mut vertices = Vec::with_capacity(vertex_count);
            for _ in 0..vertex_count {
                let position = read_f32_array3(body, cursor)?;
                cursor += 12;
                let normal = read_f32_array3(body, cursor)?;
                cursor += 12;
                let uv0 = [read_f32(body, cursor)?, read_f32(body, cursor + 4)?];
                cursor += 8;
                vertices.push(Vertex {
                    position,
                    normal,
                    uv0,
                });
            }
            let index_bytes = index_count
                .checked_mul(4)
                .ok_or("YDD index byte range overflow")?;
            if cursor
                .checked_add(index_bytes)
                .ok_or("YDD index range overflow")?
                > payload_end
            {
                return Err(format!(
                    "YDD entry '{}' mesh '{}' indices outside payload",
                    name, mesh_name
                ));
            }
            let mut indices = Vec::with_capacity(index_count);
            for _ in 0..index_count {
                indices.push(read_u32(body, cursor)?);
                cursor += 4;
            }
            meshes.push(crate::drawable::DrawableMesh {
                name: mesh_name,
                material_ref,
                vertices,
                indices,
                bounds: crate::drawable::Bounds3 {
                    min: bounds_min,
                    max: bounds_max,
                },
            });
        }
        let bounds = crate::drawable::recompute_model_bounds(&meshes);
        models.push(DrawableModel {
            name,
            source_path,
            properties_ref,
            meshes,
            bounds,
        });
    }
    Ok(DrawableDictionary::new(models))
}

fn encode_body(dict: &DrawableDictionary) -> Result<Vec<u8>, String> {
    let entry_count = dict.models.len();

    let entry_table_offset = BODY_HEADER_LEN;

    let string_table_offset = entry_table_offset + entry_count * ENTRY_RECORD_LEN;

    let mut strings = Vec::<u8>::new();

    let mut payloads = Vec::<Vec<u8>>::new();

    let mut records = Vec::<EntryBuildRecord>::new();

    for model in &dict.models {
        let name_offset = push_string(&mut strings, &model.name);

        let source_offset = push_string(&mut strings, &model.source_path);
        let properties_offset = model
            .properties_ref
            .as_deref()
            .map(|value| push_string(&mut strings, value))
            .unwrap_or(u32::MAX);

        let mut payload = Vec::new();

        write_model_payload(&mut payload, model, &mut strings)?;

        records.push(EntryBuildRecord {
            name: model.name.clone(),
            name_offset,
            source_offset,
            properties_offset,

            mesh_count: model.meshes.len() as u32,

            vertex_count: model.meshes.iter().map(|m| m.vertices.len() as u32).sum(),

            index_count: model.meshes.iter().map(|m| m.indices.len() as u32).sum(),

            material_count: model.meshes.len() as u32,

            bounds_min: model.bounds.min,
            bounds_max: model.bounds.max,

            payload_len: payload.len() as u64,
        });

        payloads.push(payload);
    }

    let payload_offset = string_table_offset + strings.len();

    let payload_len: usize = payloads.iter().map(|p| p.len()).sum();

    let mut out = Vec::with_capacity(payload_offset + payload_len);

    write_u32_vec(&mut out, BODY_SCHEMA_VERSION);

    write_u32_vec(&mut out, entry_count as u32);

    write_u64_vec(&mut out, entry_table_offset as u64);

    write_u64_vec(&mut out, string_table_offset as u64);

    write_u64_vec(&mut out, strings.len() as u64);

    write_u64_vec(&mut out, payload_offset as u64);

    let mut running_payload_offset = payload_offset as u64;

    for record in &records {
        write_u64_vec(&mut out, stable_hash64(&record.name));

        write_u32_vec(&mut out, record.name_offset);

        write_u32_vec(&mut out, record.source_offset);

        write_u32_vec(&mut out, record.mesh_count);

        write_u32_vec(&mut out, record.vertex_count);

        write_u32_vec(&mut out, record.index_count);

        write_u32_vec(&mut out, record.material_count);

        write_u32_vec(&mut out, record.properties_offset);

        write_f32_array(&mut out, record.bounds_min);

        write_f32_array(&mut out, record.bounds_max);

        write_u32_vec(&mut out, 0);

        // ENTRY_RECORD_LEN is 80. The parser expects payload_offset at +64 and payload_len at +72.

        // Keep exactly one reserved u32 after bounds; a second one shifts payload fields and corrupts self-parse.

        write_u64_vec(&mut out, running_payload_offset);

        write_u64_vec(&mut out, record.payload_len);

        running_payload_offset += record.payload_len;
    }

    debug_assert_eq!(out.len(), string_table_offset);

    out.extend_from_slice(&strings);

    for payload in payloads {
        out.extend_from_slice(&payload);
    }

    Ok(out)
}

fn parse_body_index(body: &[u8], file_name: &str) -> Result<Vec<ResidentEntryInfo>, String> {
    if body.len() < BODY_HEADER_LEN {
        return Err(format!("YDD body too small: {}", body.len()));
    }

    let version = read_u32(body, 0)?;

    if version != BODY_SCHEMA_VERSION {
        return Err(format!("unsupported YDD body schema version {version}"));
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

fn write_model_payload(
    out: &mut Vec<u8>,
    model: &DrawableModel,
    strings: &mut Vec<u8>,
) -> Result<(), String> {
    write_u32_vec(out, model.meshes.len() as u32);

    write_u32_vec(out, 0);

    for mesh in &model.meshes {
        let name_offset = push_string(strings, &mesh.name);

        let material_offset = mesh
            .material_ref
            .as_ref()
            .map(|m| push_string(strings, m))
            .unwrap_or(u32::MAX);

        write_u32_vec(out, name_offset);

        write_u32_vec(out, material_offset);

        write_u32_vec(out, mesh.vertices.len() as u32);

        write_u32_vec(out, mesh.indices.len() as u32);

        write_f32_array(out, mesh.bounds.min);

        write_f32_array(out, mesh.bounds.max);

        for vertex in &mesh.vertices {
            write_vertex(out, *vertex);
        }

        for idx in &mesh.indices {
            write_u32_vec(out, *idx);
        }
    }

    Ok(())
}

struct EntryBuildRecord {
    name: String,
    name_offset: u32,
    source_offset: u32,
    properties_offset: u32,
    mesh_count: u32,
    vertex_count: u32,
    index_count: u32,
    material_count: u32,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    payload_len: u64,
}

fn write_vertex(out: &mut Vec<u8>, v: Vertex) {
    write_f32_array(out, v.position);
    write_f32_array(out, v.normal);
    write_f32_array2(out, v.uv0);
}

fn write_f32_array(out: &mut Vec<u8>, values: [f32; 3]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn write_f32_array2(out: &mut Vec<u8>, values: [f32; 2]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn push_string(strings: &mut Vec<u8>, value: &str) -> u32 {
    let offset = strings.len() as u32;
    strings.extend_from_slice(value.as_bytes());
    strings.push(0);
    offset
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

fn write_u32_vec(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64_vec(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
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

fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let s = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated f32 at {offset}"))?;
    Ok(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_f32_array3(bytes: &[u8], offset: usize) -> Result<[f32; 3], String> {
    Ok([
        read_f32(bytes, offset)?,
        read_f32(bytes, offset + 4)?,
        read_f32(bytes, offset + 8)?,
    ])
}
