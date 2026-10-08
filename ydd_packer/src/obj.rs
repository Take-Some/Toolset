use std::fs;
use std::path::Path;

use crate::drawable::{DrawableMesh, Vertex};
use crate::model::{
    apply_position_scale, default_normal, make_mesh, make_model, source_entry_name, ImportOptions,
};

pub fn import_obj(
    path: &Path,
    options: &ImportOptions,
) -> Result<crate::drawable::DrawableModel, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("read OBJ '{}' failed: {e}", path.display()))?;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut meshes: Vec<DrawableMesh> = Vec::new();
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut mesh_name = path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("mesh")
        .to_owned();
    let mut current_material = options.fallback_material.clone();

    for (line_no, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        match parts.next().unwrap_or("") {
            "o" | "g" => {
                // OBJ object/group statements establish source mesh identity. Flush the previous
                // group before changing its name; otherwise every later group inherits the first
                // label when `g` precedes `usemtl` (the canonical NorthStar level-export layout).
                flush_mesh(
                    &mut meshes,
                    &mesh_name,
                    &current_material,
                    &mut vertices,
                    &mut indices,
                );
                if let Some(name) = parts.next() {
                    mesh_name = sanitize_local_name(name);
                }
            }
            "usemtl" => {
                flush_mesh(
                    &mut meshes,
                    &mesh_name,
                    &current_material,
                    &mut vertices,
                    &mut indices,
                );
                if let Some(name) = parts.next() {
                    current_material = Some(material_to_ref(name, &options.fallback_material));
                    if mesh_name == path.file_stem().and_then(|x| x.to_str()).unwrap_or("mesh") {
                        mesh_name = sanitize_local_name(name);
                    }
                }
            }
            "v" => {
                let x = parse_f32(parts.next(), path, line_no, "v.x")?;
                let y = parse_f32(parts.next(), path, line_no, "v.y")?;
                let z = parse_f32(parts.next(), path, line_no, "v.z")?;
                positions.push(apply_position_scale([x, y, z], options.scale));
            }
            "vn" => {
                let x = parse_f32(parts.next(), path, line_no, "vn.x")?;
                let y = parse_f32(parts.next(), path, line_no, "vn.y")?;
                let z = parse_f32(parts.next(), path, line_no, "vn.z")?;
                normals.push([x, y, z]);
            }
            "vt" => {
                let u = parse_f32(parts.next(), path, line_no, "vt.u")?;
                let mut v = parse_f32(parts.next(), path, line_no, "vt.v")?;
                if options.flip_v {
                    v = 1.0 - v;
                }
                uvs.push([u, v]);
            }
            "f" => {
                let face = parts
                    .map(|p| {
                        parse_face_vertex(
                            p,
                            positions.len(),
                            uvs.len(),
                            normals.len(),
                            path,
                            line_no,
                        )
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                if face.len() < 3 {
                    return Err(format!(
                        "OBJ '{}' line {} face has less than 3 vertices",
                        path.display(),
                        line_no + 1
                    ));
                }
                if face.len() > 3 && !options.triangulate {
                    return Err(format!(
                        "OBJ '{}' line {} requires triangulation",
                        path.display(),
                        line_no + 1
                    ));
                }
                for tri in triangulate_fan(&face) {
                    // OBJ NORMAL is optional. A constant +Y fallback is only correct
                    // for horizontal faces and makes side-face lighting depend on the
                    // camera/view vector. Generate the geometric triangle normal when
                    // the source omits vn so packed YDD lighting stays camera-invariant.
                    let generated_normal =
                        triangle_face_normal(&tri, &positions).unwrap_or_else(default_normal);
                    for fv in tri {
                        let pos = positions.get(fv.position).copied().ok_or_else(|| {
                            format!(
                                "OBJ '{}' line {} position index outside range",
                                path.display(),
                                line_no + 1
                            )
                        })?;
                        let uv = fv
                            .uv
                            .and_then(|i| uvs.get(i).copied())
                            .unwrap_or([0.0, 0.0]);
                        let normal = fv
                            .normal
                            .and_then(|i| normals.get(i).copied())
                            .unwrap_or(generated_normal);
                        vertices.push(Vertex {
                            position: pos,
                            normal,
                            uv0: uv,
                        });
                        indices.push((vertices.len() - 1) as u32);
                    }
                }
            }
            _ => {}
        }
    }
    flush_mesh(
        &mut meshes,
        &mesh_name,
        &current_material,
        &mut vertices,
        &mut indices,
    );

    if meshes.is_empty() {
        return Err(format!("OBJ '{}' produced no triangles", path.display()));
    }
    Ok(make_model(source_entry_name(path, options), path, meshes))
}

fn flush_mesh(
    meshes: &mut Vec<DrawableMesh>,
    name: &str,
    material_ref: &Option<String>,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
) {
    if vertices.is_empty() || indices.is_empty() {
        return;
    }
    let mut mesh_name = sanitize_local_name(name);
    if let Some(material) = material_ref.as_deref() {
        if let Some((_, selector)) = material.rsplit_once('@') {
            let selector = sanitize_local_name(selector);
            if !selector.is_empty() && mesh_name != selector {
                mesh_name = format!("{}_{}", mesh_name, selector);
            }
        }
    }
    meshes.push(make_mesh(
        mesh_name,
        material_ref.clone(),
        std::mem::take(vertices),
        std::mem::take(indices),
    ));
}

#[derive(Debug, Clone, Copy)]
struct FaceVertex {
    position: usize,
    uv: Option<usize>,
    normal: Option<usize>,
}

fn parse_face_vertex(
    token: &str,
    pos_len: usize,
    uv_len: usize,
    normal_len: usize,
    path: &Path,
    line_no: usize,
) -> Result<FaceVertex, String> {
    let mut parts = token.split('/');
    let pos = parse_index(
        parts.next().unwrap_or(""),
        pos_len,
        path,
        line_no,
        "position",
    )?;
    let uv = match parts.next() {
        Some("") | None => None,
        Some(v) => Some(parse_index(v, uv_len, path, line_no, "uv")?),
    };
    let normal = match parts.next() {
        Some("") | None => None,
        Some(v) => Some(parse_index(v, normal_len, path, line_no, "normal")?),
    };
    Ok(FaceVertex {
        position: pos,
        uv,
        normal,
    })
}

fn triangulate_fan(face: &[FaceVertex]) -> Vec<[FaceVertex; 3]> {
    let mut out = Vec::new();
    for i in 1..face.len() - 1 {
        out.push([face[0], face[i], face[i + 1]]);
    }
    out
}

fn triangle_face_normal(tri: &[FaceVertex; 3], positions: &[[f32; 3]]) -> Option<[f32; 3]> {
    let a = *positions.get(tri[0].position)?;
    let b = *positions.get(tri[1].position)?;
    let c = *positions.get(tri[2].position)?;
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let len2 = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
    if !len2.is_finite() || len2 <= 1.0e-20 {
        return None;
    }
    let inv_len = len2.sqrt().recip();
    Some([n[0] * inv_len, n[1] * inv_len, n[2] * inv_len])
}

fn parse_index(
    raw: &str,
    len: usize,
    path: &Path,
    line_no: usize,
    field: &str,
) -> Result<usize, String> {
    let idx = raw.parse::<isize>().map_err(|_| {
        format!(
            "OBJ '{}' line {} invalid {field} index '{raw}'",
            path.display(),
            line_no + 1
        )
    })?;
    if idx == 0 {
        return Err(format!(
            "OBJ '{}' line {} {field} index is 1-based and cannot be 0",
            path.display(),
            line_no + 1
        ));
    }
    let resolved = if idx < 0 { len as isize + idx } else { idx - 1 };
    if resolved < 0 || resolved as usize >= len {
        return Err(format!(
            "OBJ '{}' line {} {field} index outside range",
            path.display(),
            line_no + 1
        ));
    }
    Ok(resolved as usize)
}

fn parse_f32(raw: Option<&str>, path: &Path, line_no: usize, field: &str) -> Result<f32, String> {
    raw.ok_or_else(|| {
        format!(
            "OBJ '{}' line {} missing {field}",
            path.display(),
            line_no + 1
        )
    })?
    .parse::<f32>()
    .map_err(|_| {
        format!(
            "OBJ '{}' line {} invalid {field}",
            path.display(),
            line_no + 1
        )
    })
}

fn sanitize_local_name(value: &str) -> String {
    crate::drawable::sanitize_entry_name(value)
}

fn material_to_ref(name: &str, fallback: &Option<String>) -> String {
    if name.contains(".ymat") {
        name.to_owned()
    } else if let Some(base) = fallback {
        if base.contains('@') {
            base.clone()
        } else {
            format!(
                "{}@{}",
                base.trim_end_matches('@'),
                crate::drawable::sanitize_entry_name(name)
            )
        }
    } else {
        let name = crate::drawable::sanitize_entry_name(name);
        format!("materials/{name}.ymat@{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_obj_normals_use_geometric_face_normal() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "northstar-ydd-missing-normal-{}-{stamp}.obj",
            std::process::id()
        ));
        // Vertical YZ triangle. The old +Y fallback was geometrically wrong;
        // its actual winding normal is +X.
        fs::write(
            &path,
            "v 0 0 0\nv 0 1 0\nv 0 0 1\nvt 0 0\nvt 1 0\nvt 0 1\nf 1/1 2/2 3/3\n",
        )
        .expect("write OBJ fixture");
        let options = ImportOptions {
            scale: 1.0,
            flip_v: false,
            triangulate: true,
            fallback_material: None,
            properties_ref: None,
            explicit_entry_name: None,
        };
        let model = import_obj(&path, &options).expect("import OBJ without vn");
        let _ = fs::remove_file(&path);
        let mesh = model.meshes.first().expect("mesh");
        assert_eq!(mesh.vertices.len(), 3);
        for vertex in &mesh.vertices {
            assert!(
                (vertex.normal[0] - 1.0).abs() < 1.0e-5,
                "normal={:?}",
                vertex.normal
            );
            assert!(
                vertex.normal[1].abs() < 1.0e-5,
                "normal={:?}",
                vertex.normal
            );
            assert!(
                vertex.normal[2].abs() < 1.0e-5,
                "normal={:?}",
                vertex.normal
            );
        }
    }
    #[test]
    fn obj_groups_preserve_independent_mesh_identity() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "northstar-ydd-groups-{}-{stamp}.obj",
            std::process::id()
        ));
        fs::write(
            &path,
            "g first\nusemtl mat_a\nv 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nvn 0 0 1\nf 1/1/1 2/2/1 3/3/1\ng second\nusemtl mat_b\nv 2 0 0\nv 3 0 0\nv 2 1 0\nf 4/1/1 5/2/1 6/3/1\n",
        )
        .expect("write grouped OBJ fixture");
        let options = ImportOptions {
            scale: 1.0,
            flip_v: false,
            triangulate: true,
            fallback_material: None,
            properties_ref: None,
            explicit_entry_name: None,
        };
        let model = import_obj(&path, &options).expect("import grouped OBJ");
        let _ = fs::remove_file(&path);
        assert_eq!(model.meshes.len(), 2);
        assert!(
            model.meshes[0].name.starts_with("first"),
            "{:?}",
            model.meshes[0].name
        );
        assert!(
            model.meshes[1].name.starts_with("second"),
            "{:?}",
            model.meshes[1].name
        );
    }
}
