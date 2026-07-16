use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Deserialize;

use crate::drawable::Vertex;
use crate::model::{
    apply_position_scale, default_normal, make_mesh, make_model, source_entry_name, ImportOptions,
};

pub fn import_gltf(
    path: &Path,
    options: &ImportOptions,
) -> Result<Vec<crate::drawable::DrawableModel>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("read glTF '{}' failed: {e}", path.display()))?;
    let doc: GltfDoc = serde_json::from_str(&text)
        .map_err(|e| format!("parse glTF '{}' failed: {e}", path.display()))?;
    let buffers = load_gltf_buffers(path, &doc)?;
    import_doc(path, &doc, &buffers, None, options)
}

pub fn import_glb(
    path: &Path,
    options: &ImportOptions,
) -> Result<Vec<crate::drawable::DrawableModel>, String> {
    let bytes = fs::read(path).map_err(|e| format!("read GLB '{}' failed: {e}", path.display()))?;
    let (json_chunk, bin_chunk) = read_glb_chunks(&bytes, path)?;
    let doc: GltfDoc = serde_json::from_slice(json_chunk)
        .map_err(|e| format!("parse GLB JSON '{}' failed: {e}", path.display()))?;
    let buffers = if let Some(bin) = bin_chunk {
        vec![bin.to_vec()]
    } else {
        load_gltf_buffers(path, &doc)?
    };
    import_doc(path, &doc, &buffers, bin_chunk, options)
}

fn import_doc(
    path: &Path,
    doc: &GltfDoc,
    buffers: &[Vec<u8>],
    _bin: Option<&[u8]>,
    options: &ImportOptions,
) -> Result<Vec<crate::drawable::DrawableModel>, String> {
    let entry_name = source_entry_name(path, options);
    let mut meshes = Vec::new();
    for (mesh_index, mesh) in doc.meshes.as_deref().unwrap_or(&[]).iter().enumerate() {
        for (prim_index, prim) in mesh.primitives.as_deref().unwrap_or(&[]).iter().enumerate() {
            let positions =
                read_vec3_accessor(doc, buffers, prim.attributes.position, options.scale)?;
            if positions.is_empty() {
                continue;
            }
            let normals = match prim.attributes.normal {
                Some(i) => read_vec3_accessor(doc, buffers, i, 1.0)?,
                None => vec![default_normal(); positions.len()],
            };
            let mut uvs = match prim.attributes.texcoord_0 {
                Some(i) => read_vec2_accessor(doc, buffers, i)?,
                None => vec![[0.0, 0.0]; positions.len()],
            };
            if options.flip_v {
                for uv in &mut uvs {
                    uv[1] = 1.0 - uv[1];
                }
            }
            let count = positions.len().min(normals.len()).min(uvs.len());
            let mut vertices = Vec::with_capacity(count);
            for i in 0..count {
                vertices.push(Vertex {
                    position: positions[i],
                    normal: normals[i],
                    uv0: uvs[i],
                });
            }
            let indices = if let Some(i) = prim.indices {
                read_index_accessor(doc, buffers, i)?
            } else {
                sequential_tris(count)
            };
            if indices.len() < 3 {
                continue;
            }
            if indices.len() % 3 != 0 && !options.triangulate {
                return Err(format!("glTF '{}' mesh {mesh_index} primitive {prim_index} has non-triangle index count", path.display()));
            }
            let mesh_name = mesh
                .name
                .clone()
                .unwrap_or_else(|| format!("mesh_{mesh_index}_{prim_index}"));
            meshes.push(make_mesh(
                crate::drawable::sanitize_entry_name(&mesh_name),
                options.fallback_material.clone(),
                vertices,
                trim_to_triangles(indices),
            ));
        }
    }
    if meshes.is_empty() {
        return Err(format!(
            "glTF '{}' contains no importable triangle primitives",
            path.display()
        ));
    }
    Ok(vec![make_model(entry_name, path, meshes)])
}

fn load_gltf_buffers(path: &Path, doc: &GltfDoc) -> Result<Vec<Vec<u8>>, String> {
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = Vec::new();
    for buffer in doc.buffers.as_deref().unwrap_or(&[]) {
        let uri = buffer
            .uri
            .as_deref()
            .ok_or_else(|| format!("glTF '{}' has external buffer without uri", path.display()))?;
        if let Some(rest) = uri
            .strip_prefix("data:application/octet-stream;base64,")
            .or_else(|| uri.strip_prefix("data:application/gltf-buffer;base64,"))
        {
            out.push(
                base64::engine::general_purpose::STANDARD
                    .decode(rest)
                    .map_err(|e| format!("decode embedded glTF buffer failed: {e}"))?,
            );
        } else {
            let p = normalize_join(base, uri);
            out.push(
                fs::read(&p)
                    .map_err(|e| format!("read glTF buffer '{}' failed: {e}", p.display()))?,
            );
        }
    }
    Ok(out)
}

fn read_vec3_accessor(
    doc: &GltfDoc,
    buffers: &[Vec<u8>],
    accessor_index: usize,
    scale: f32,
) -> Result<Vec<[f32; 3]>, String> {
    let view = accessor_view(doc, buffers, accessor_index)?;
    if view.accessor.component_type != 5126 || view.accessor.type_name.as_deref() != Some("VEC3") {
        return Err(format!("glTF accessor {accessor_index} must be FLOAT VEC3"));
    }
    let stride = view.stride.unwrap_or(12);
    let mut out = Vec::with_capacity(view.accessor.count);
    for i in 0..view.accessor.count {
        let o = view.offset + i * stride;
        out.push(apply_position_scale(
            [
                read_f32(view.bytes, o)?,
                read_f32(view.bytes, o + 4)?,
                read_f32(view.bytes, o + 8)?,
            ],
            scale,
        ));
    }
    Ok(out)
}

fn read_vec2_accessor(
    doc: &GltfDoc,
    buffers: &[Vec<u8>],
    accessor_index: usize,
) -> Result<Vec<[f32; 2]>, String> {
    let view = accessor_view(doc, buffers, accessor_index)?;
    if view.accessor.component_type != 5126 || view.accessor.type_name.as_deref() != Some("VEC2") {
        return Err(format!("glTF accessor {accessor_index} must be FLOAT VEC2"));
    }
    let stride = view.stride.unwrap_or(8);
    let mut out = Vec::with_capacity(view.accessor.count);
    for i in 0..view.accessor.count {
        let o = view.offset + i * stride;
        out.push([read_f32(view.bytes, o)?, read_f32(view.bytes, o + 4)?]);
    }
    Ok(out)
}

fn read_index_accessor(
    doc: &GltfDoc,
    buffers: &[Vec<u8>],
    accessor_index: usize,
) -> Result<Vec<u32>, String> {
    let view = accessor_view(doc, buffers, accessor_index)?;
    let size = match view.accessor.component_type {
        5121 => 1,
        5123 => 2,
        5125 => 4,
        other => {
            return Err(format!(
                "glTF index accessor {accessor_index} unsupported componentType {other}"
            ))
        }
    };
    let stride = view.stride.unwrap_or(size);
    let mut out = Vec::with_capacity(view.accessor.count);
    for i in 0..view.accessor.count {
        let o = view.offset + i * stride;
        out.push(match size {
            1 => *view.bytes.get(o).ok_or("glTF index u8 outside buffer")? as u32,
            2 => read_u16(view.bytes, o)? as u32,
            4 => read_u32(view.bytes, o)?,
            _ => unreachable!(),
        });
    }
    Ok(out)
}

struct AccessorView<'a> {
    accessor: &'a GltfAccessor,
    bytes: &'a [u8],
    offset: usize,
    stride: Option<usize>,
}

fn accessor_view<'a>(
    doc: &'a GltfDoc,
    buffers: &'a [Vec<u8>],
    accessor_index: usize,
) -> Result<AccessorView<'a>, String> {
    let accessor = doc
        .accessors
        .as_ref()
        .and_then(|a| a.get(accessor_index))
        .ok_or_else(|| format!("glTF accessor {accessor_index} missing"))?;
    let view_index = accessor
        .buffer_view
        .ok_or_else(|| format!("glTF accessor {accessor_index} has no bufferView"))?;
    let bv = doc
        .buffer_views
        .as_ref()
        .and_then(|v| v.get(view_index))
        .ok_or_else(|| format!("glTF bufferView {view_index} missing"))?;
    let buffer = buffers
        .get(bv.buffer)
        .ok_or_else(|| format!("glTF buffer {} missing", bv.buffer))?;
    let view_offset = bv.byte_offset.unwrap_or(0);
    let accessor_offset = accessor.byte_offset.unwrap_or(0);
    let offset = view_offset + accessor_offset;
    Ok(AccessorView {
        accessor,
        bytes: buffer,
        offset,
        stride: bv.byte_stride,
    })
}

fn read_glb_chunks<'a>(
    bytes: &'a [u8],
    path: &Path,
) -> Result<(&'a [u8], Option<&'a [u8]>), String> {
    if bytes.len() < 20 {
        return Err(format!("GLB '{}' too small", path.display()));
    }
    if bytes.get(0..4) != Some(b"glTF") {
        return Err(format!("GLB '{}' magic mismatch", path.display()));
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != 2 {
        return Err(format!(
            "GLB '{}' unsupported version {version}",
            path.display()
        ));
    }
    let mut cursor = 12usize;
    let mut json = None;
    let mut bin = None;
    while cursor + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        let ty = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap());
        let start = cursor + 8;
        let end = start.checked_add(len).ok_or("GLB chunk overflow")?;
        let chunk = bytes
            .get(start..end)
            .ok_or_else(|| format!("GLB '{}' chunk outside file", path.display()))?;
        match ty {
            0x4E4F534A => json = Some(chunk),
            0x004E4942 => bin = Some(chunk),
            _ => {}
        }
        cursor = align4(end);
    }
    Ok((
        json.ok_or_else(|| format!("GLB '{}' has no JSON chunk", path.display()))?,
        bin,
    ))
}

fn sequential_tris(count: usize) -> Vec<u32> {
    (0..(count / 3 * 3)).map(|i| i as u32).collect()
}
fn trim_to_triangles(mut values: Vec<u32>) -> Vec<u32> {
    values.truncate(values.len() / 3 * 3);
    values
}
fn normalize_join(base: &Path, uri: &str) -> PathBuf {
    base.join(uri.replace('%', "_"))
}
fn align4(v: usize) -> usize {
    (v + 3) & !3
}
fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let s = bytes
        .get(offset..offset + 4)
        .ok_or("glTF f32 outside buffer")?;
    Ok(f32::from_le_bytes(s.try_into().unwrap()))
}
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let s = bytes
        .get(offset..offset + 2)
        .ok_or("glTF u16 outside buffer")?;
    Ok(u16::from_le_bytes(s.try_into().unwrap()))
}
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let s = bytes
        .get(offset..offset + 4)
        .ok_or("glTF u32 outside buffer")?;
    Ok(u32::from_le_bytes(s.try_into().unwrap()))
}

#[derive(Debug, Deserialize)]
struct GltfDoc {
    buffers: Option<Vec<GltfBuffer>>,
    #[serde(rename = "bufferViews")]
    buffer_views: Option<Vec<GltfBufferView>>,
    accessors: Option<Vec<GltfAccessor>>,
    meshes: Option<Vec<GltfMesh>>,
}
#[derive(Debug, Deserialize)]
struct GltfBuffer {
    uri: Option<String>,
    #[serde(rename = "byteLength")]
    _byte_length: Option<usize>,
}
#[derive(Debug, Deserialize)]
struct GltfBufferView {
    buffer: usize,
    #[serde(rename = "byteOffset")]
    byte_offset: Option<usize>,
    #[serde(rename = "byteLength")]
    _byte_length: usize,
    #[serde(rename = "byteStride")]
    byte_stride: Option<usize>,
}
#[derive(Debug, Deserialize)]
struct GltfAccessor {
    #[serde(rename = "bufferView")]
    buffer_view: Option<usize>,
    #[serde(rename = "byteOffset")]
    byte_offset: Option<usize>,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    type_name: Option<String>,
}
#[derive(Debug, Deserialize)]
struct GltfMesh {
    name: Option<String>,
    primitives: Option<Vec<GltfPrimitive>>,
}
#[derive(Debug, Deserialize)]
struct GltfPrimitive {
    attributes: GltfAttributes,
    indices: Option<usize>,
}
#[derive(Debug, Deserialize)]
struct GltfAttributes {
    #[serde(rename = "POSITION")]
    position: usize,
    #[serde(rename = "NORMAL")]
    normal: Option<usize>,
    #[serde(rename = "TEXCOORD_0")]
    texcoord_0: Option<usize>,
}
