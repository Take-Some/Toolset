use std::fs;
use std::io::Read;
use std::path::Path;

use flate2::read::ZlibDecoder;

use crate::drawable::Vertex;
use crate::model::{
    apply_position_scale, default_normal, make_mesh, make_model, source_entry_name, ImportOptions,
};

const FBX_BINARY_MAGIC: &[u8] = b"Kaydara FBX Binary  \0\x1a\0";

pub fn import_fbx(
    path: &Path,
    options: &ImportOptions,
) -> Result<Vec<crate::drawable::DrawableModel>, String> {
    let bytes = fs::read(path).map_err(|e| format!("read FBX '{}' failed: {e}", path.display()))?;
    let mesh = if bytes.starts_with(FBX_BINARY_MAGIC) {
        import_binary_fbx(path, &bytes, options)?
    } else {
        import_ascii_fbx(path, &bytes, options)?
    };
    Ok(vec![mesh])
}

fn import_ascii_fbx(
    path: &Path,
    bytes: &[u8],
    options: &ImportOptions,
) -> Result<crate::drawable::DrawableModel, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        format!(
            "FBX '{}' is neither binary FBX nor UTF-8 ASCII FBX",
            path.display()
        )
    })?;
    let vertices = read_ascii_f64_array(text, "Vertices")?;
    let poly = read_ascii_i32_array(text, "PolygonVertexIndex")?;
    let normals = read_ascii_f64_array(text, "Normals").ok();
    let uv = read_ascii_f64_array(text, "UV").ok();
    build_model(path, options, vertices, poly, normals, uv)
}

fn read_ascii_f64_array(text: &str, label: &str) -> Result<Vec<f64>, String> {
    let block = ascii_array_block(text, label)?;
    let out = tokens(block)
        .into_iter()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect::<Vec<_>>();
    if out.is_empty() {
        Err(format!("FBX array '{label}' is empty"))
    } else {
        Ok(out)
    }
}

fn read_ascii_i32_array(text: &str, label: &str) -> Result<Vec<i32>, String> {
    let block = ascii_array_block(text, label)?;
    let out = tokens(block)
        .into_iter()
        .filter_map(|t| t.parse::<i32>().ok())
        .collect::<Vec<_>>();
    if out.is_empty() {
        Err(format!("FBX array '{label}' is empty"))
    } else {
        Ok(out)
    }
}

fn ascii_array_block<'a>(text: &'a str, label: &str) -> Result<&'a str, String> {
    let needle = format!("{label}:");
    let start = text
        .find(&needle)
        .ok_or_else(|| format!("FBX missing '{label}' array"))?
        + needle.len();
    let rest = &text[start..];
    if let Some(a) = rest.find("a:") {
        let data = &rest[a + 2..];
        Ok(&data[..data.find('}').unwrap_or(data.len())])
    } else {
        Ok(&rest[..rest.find('\n').unwrap_or(rest.len())])
    }
}

fn tokens(input: &str) -> Vec<&str> {
    input
        .split(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')))
        .filter(|v| v.chars().any(|c| c.is_ascii_digit()))
        .collect()
}

fn import_binary_fbx(
    path: &Path,
    bytes: &[u8],
    options: &ImportOptions,
) -> Result<crate::drawable::DrawableModel, String> {
    if bytes.len() < 27 {
        return Err(format!(
            "FBX binary '{}' header is truncated",
            path.display()
        ));
    }
    let version = read_u32_at(bytes, 23)?;
    let reader = BinaryReader { bytes, version };
    let mut geo = GeometryArrays::default();
    let mut offset = 27usize;
    while offset < bytes.len() {
        let Some(node) = reader.read_node(offset)? else {
            break;
        };
        collect_geometry(&reader, &node, &mut geo)?;
        if geo.vertices.is_some() && geo.polygon_indices.is_some() {
            break;
        }
        if node.end_offset <= offset {
            break;
        }
        offset = node.end_offset;
    }
    let vertices = geo
        .vertices
        .ok_or_else(|| format!("FBX binary '{}' missing Vertices array", path.display()))?;
    let poly = geo.polygon_indices.ok_or_else(|| {
        format!(
            "FBX binary '{}' missing PolygonVertexIndex array",
            path.display()
        )
    })?;
    build_model(path, options, vertices, poly, geo.normals, geo.uv)
}

#[derive(Default)]
struct GeometryArrays {
    vertices: Option<Vec<f64>>,
    polygon_indices: Option<Vec<i32>>,
    normals: Option<Vec<f64>>,
    uv: Option<Vec<f64>>,
}

fn collect_geometry(
    reader: &BinaryReader<'_>,
    node: &Node,
    geo: &mut GeometryArrays,
) -> Result<(), String> {
    match node.name.as_str() {
        "Vertices" if geo.vertices.is_none() => {
            if let Some(Property::F64Array(values)) = node.properties.first() {
                geo.vertices = Some(values.clone());
            }
        }
        "PolygonVertexIndex" if geo.polygon_indices.is_none() => {
            if let Some(Property::I32Array(values)) = node.properties.first() {
                geo.polygon_indices = Some(values.clone());
            }
        }
        "Normals" if geo.normals.is_none() => {
            if let Some(Property::F64Array(values)) = node.properties.first() {
                geo.normals = Some(values.clone());
            }
        }
        "UV" if geo.uv.is_none() => {
            if let Some(Property::F64Array(values)) = node.properties.first() {
                geo.uv = Some(values.clone());
            }
        }
        _ => {}
    }

    let mut child_offset = node.children_start;
    while child_offset < node.children_end {
        let Some(child) = reader.read_node(child_offset)? else {
            break;
        };
        collect_geometry(reader, &child, geo)?;
        if child.end_offset <= child_offset {
            break;
        }
        child_offset = child.end_offset;
    }
    Ok(())
}

fn build_model(
    path: &Path,
    options: &ImportOptions,
    raw: Vec<f64>,
    encoded: Vec<i32>,
    raw_normals: Option<Vec<f64>>,
    raw_uv: Option<Vec<f64>>,
) -> Result<crate::drawable::DrawableModel, String> {
    if raw.len() % 3 != 0 {
        return Err(format!(
            "FBX '{}' vertex array is not xyz triples",
            path.display()
        ));
    }
    let positions = raw
        .chunks_exact(3)
        .map(|v| apply_position_scale([v[0] as f32, v[1] as f32, v[2] as f32], options.scale))
        .collect::<Vec<_>>();
    if positions.is_empty() {
        return Err(format!("FBX '{}' contains no positions", path.display()));
    }

    let normals = raw_normals.and_then(|values| {
        if values.len() % 3 == 0 && !values.is_empty() {
            Some(
                values
                    .chunks_exact(3)
                    .map(|v| normalize_or_default([v[0] as f32, v[1] as f32, v[2] as f32]))
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        }
    });
    let uvs = raw_uv.and_then(|values| {
        if values.len() % 2 == 0 && !values.is_empty() {
            Some(
                values
                    .chunks_exact(2)
                    .map(|v| {
                        [
                            v[0] as f32,
                            if options.flip_v {
                                1.0 - v[1] as f32
                            } else {
                                v[1] as f32
                            },
                        ]
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        }
    });

    let mut verts = Vec::new();
    let mut inds = Vec::new();
    let mut face = Vec::new();
    for code in encoded {
        let end = code < 0;
        let idx = if end {
            (-code - 1) as usize
        } else {
            code as usize
        };
        if idx >= positions.len() {
            return Err(format!(
                "FBX '{}' polygon index {idx} outside vertex count {}",
                path.display(),
                positions.len()
            ));
        }
        face.push(idx);
        if end {
            emit_face(
                &face,
                &positions,
                normals.as_deref(),
                uvs.as_deref(),
                &mut verts,
                &mut inds,
                options.triangulate,
            )?;
            face.clear();
        }
    }
    if !face.is_empty() {
        emit_face(
            &face,
            &positions,
            normals.as_deref(),
            uvs.as_deref(),
            &mut verts,
            &mut inds,
            options.triangulate,
        )?;
    }
    if verts.is_empty() {
        return Err(format!("FBX '{}' produced no triangles", path.display()));
    }

    let name = source_entry_name(path, options);
    let mesh = make_mesh(
        format!("{name}_mesh"),
        options.fallback_material.clone(),
        verts,
        inds,
    );
    Ok(make_model(name, path, vec![mesh]))
}

fn emit_face(
    face: &[usize],
    positions: &[[f32; 3]],
    normals: Option<&[[f32; 3]]>,
    uvs: Option<&[[f32; 2]]>,
    verts: &mut Vec<Vertex>,
    inds: &mut Vec<u32>,
    triangulate: bool,
) -> Result<(), String> {
    if face.len() < 3 {
        return Ok(());
    }
    if face.len() != 3 && !triangulate {
        return Err(
            "FBX contains non-triangle polygon and --no-triangulate was requested".to_owned(),
        );
    }
    for i in 1..face.len() - 1 {
        emit_tri(
            [face[0], face[i], face[i + 1]],
            positions,
            normals,
            uvs,
            verts,
            inds,
        );
    }
    Ok(())
}

fn emit_tri(
    ix: [usize; 3],
    positions: &[[f32; 3]],
    normals: Option<&[[f32; 3]]>,
    uvs: Option<&[[f32; 2]]>,
    verts: &mut Vec<Vertex>,
    inds: &mut Vec<u32>,
) {
    let face_n = face_normal(positions[ix[0]], positions[ix[1]], positions[ix[2]])
        .unwrap_or_else(default_normal);
    for i in ix {
        let next = verts.len() as u32;
        let normal = normals.and_then(|n| n.get(i).copied()).unwrap_or(face_n);
        let uv0 = uvs.and_then(|u| u.get(i).copied()).unwrap_or([0.0, 0.0]);
        verts.push(Vertex {
            position: positions[i],
            normal,
            uv0,
        });
        inds.push(next);
    }
}

fn normalize_or_default(n: [f32; 3]) -> [f32; 3] {
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len <= f32::EPSILON {
        default_normal()
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Option<[f32; 3]> {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l <= f32::EPSILON {
        None
    } else {
        Some([n[0] / l, n[1] / l, n[2] / l])
    }
}

struct BinaryReader<'a> {
    bytes: &'a [u8],
    version: u32,
}

struct Node {
    name: String,
    end_offset: usize,
    properties: Vec<Property>,
    children_start: usize,
    children_end: usize,
}

#[derive(Clone)]
enum Property {
    I32Array(Vec<i32>),
    F64Array(Vec<f64>),
    Other,
}

impl<'a> BinaryReader<'a> {
    fn read_node(&self, offset: usize) -> Result<Option<Node>, String> {
        let wide = self.version >= 7500;
        let header_len = if wide { 25 } else { 13 };
        if offset + header_len > self.bytes.len() {
            return Ok(None);
        }
        let (end_offset, prop_count, prop_len, name_len, mut cursor) = if wide {
            (
                read_u64_at(self.bytes, offset)? as usize,
                read_u64_at(self.bytes, offset + 8)? as usize,
                read_u64_at(self.bytes, offset + 16)? as usize,
                self.bytes[offset + 24] as usize,
                offset + 25,
            )
        } else {
            (
                read_u32_at(self.bytes, offset)? as usize,
                read_u32_at(self.bytes, offset + 4)? as usize,
                read_u32_at(self.bytes, offset + 8)? as usize,
                self.bytes[offset + 12] as usize,
                offset + 13,
            )
        };
        if end_offset == 0 {
            return Ok(None);
        }
        if end_offset > self.bytes.len() || cursor + name_len > self.bytes.len() {
            return Err(format!(
                "FBX node range outside file offset={offset} end={end_offset}"
            ));
        }
        let name = String::from_utf8_lossy(&self.bytes[cursor..cursor + name_len]).to_string();
        cursor += name_len;
        let prop_end = cursor
            .checked_add(prop_len)
            .ok_or("FBX property range overflow")?;
        if prop_end > end_offset || prop_end > self.bytes.len() {
            return Err(format!("FBX property range outside node '{name}'"));
        }
        let mut properties = Vec::with_capacity(prop_count);
        for _ in 0..prop_count {
            let (prop, next) = self.read_property(cursor)?;
            properties.push(prop);
            cursor = next;
            if cursor > prop_end {
                return Err(format!("FBX property overread in node '{name}'"));
            }
        }
        let null_record_len = if wide { 25 } else { 13 };
        let children_end = end_offset.saturating_sub(null_record_len);
        Ok(Some(Node {
            name,
            end_offset,
            properties,
            children_start: prop_end,
            children_end,
        }))
    }

    fn read_property(&self, offset: usize) -> Result<(Property, usize), String> {
        let tag = *self
            .bytes
            .get(offset)
            .ok_or("FBX property tag outside file")? as char;
        let cursor = offset + 1;
        match tag {
            'i' => Ok((
                Property::I32Array(read_i32_array(self.bytes, cursor)?),
                skip_array(self.bytes, cursor)?,
            )),
            'd' => Ok((
                Property::F64Array(read_f64_array(self.bytes, cursor)?),
                skip_array(self.bytes, cursor)?,
            )),
            'I' => Ok((Property::Other, cursor + 4)),
            'L' => Ok((Property::Other, cursor + 8)),
            'F' => Ok((Property::Other, cursor + 4)),
            'D' => Ok((Property::Other, cursor + 8)),
            'C' => Ok((Property::Other, cursor + 1)),
            'Y' => Ok((Property::Other, cursor + 2)),
            'S' | 'R' => {
                let len = read_u32_at(self.bytes, cursor)? as usize;
                Ok((Property::Other, cursor + 4 + len))
            }
            other => Err(format!(
                "unsupported FBX binary property tag '{other}' at {offset}"
            )),
        }
    }
}

fn read_i32_array(bytes: &[u8], offset: usize) -> Result<Vec<i32>, String> {
    let (len, encoding, byte_len, data) = array_header(bytes, offset)?;
    let payload = array_payload(bytes, encoding, byte_len, data)?;
    if payload.len() != len * 4 {
        return Err("FBX Int32 array byte length mismatch".to_owned());
    }
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        out.push(read_i32_at(&payload, i * 4)?);
    }
    Ok(out)
}

fn read_f64_array(bytes: &[u8], offset: usize) -> Result<Vec<f64>, String> {
    let (len, encoding, byte_len, data) = array_header(bytes, offset)?;
    let payload = array_payload(bytes, encoding, byte_len, data)?;
    if payload.len() != len * 8 {
        return Err("FBX Float64 array byte length mismatch".to_owned());
    }
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        out.push(read_f64_at(&payload, i * 8)?);
    }
    Ok(out)
}

fn array_header(bytes: &[u8], offset: usize) -> Result<(usize, u32, usize, usize), String> {
    let len = read_u32_at(bytes, offset)? as usize;
    let encoding = read_u32_at(bytes, offset + 4)?;
    let byte_len = read_u32_at(bytes, offset + 8)? as usize;
    let data = offset + 12;
    if data + byte_len > bytes.len() {
        return Err("FBX array payload outside file".to_owned());
    }
    Ok((len, encoding, byte_len, data))
}

fn array_payload(
    bytes: &[u8],
    encoding: u32,
    byte_len: usize,
    data: usize,
) -> Result<Vec<u8>, String> {
    let slice = bytes
        .get(data..data + byte_len)
        .ok_or("FBX array payload outside file")?;
    match encoding {
        0 => Ok(slice.to_vec()),
        1 => {
            let mut decoder = ZlibDecoder::new(slice);
            let mut out = Vec::new();
            decoder
                .read_to_end(&mut out)
                .map_err(|e| format!("FBX zlib array decode failed: {e}"))?;
            Ok(out)
        }
        other => Err(format!("unsupported FBX array encoding {other}")),
    }
}

fn skip_array(bytes: &[u8], offset: usize) -> Result<usize, String> {
    let byte_len = read_u32_at(bytes, offset + 8)? as usize;
    Ok(offset + 12 + byte_len)
}

fn read_u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let s = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated u32 at {offset}"))?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_i32_at(bytes: &[u8], offset: usize) -> Result<i32, String> {
    let s = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated i32 at {offset}"))?;
    Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u64_at(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let s = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| format!("truncated u64 at {offset}"))?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

fn read_f64_at(bytes: &[u8], offset: usize) -> Result<f64, String> {
    let s = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| format!("truncated f64 at {offset}"))?;
    Ok(f64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
