//! Small static-triangle OBJ/glTF loader; no network, image decoding or deformation.
use super::{Mesh, error};
use crate::evidence::analysis::Resource;
use base64::Engine;
use parry3d_f64::na::{Matrix4, Point3, Quaternion, UnitQuaternion, Vector3};
use sha2::{Digest, Sha256};
use std::path::{Component, Path};
const MAX_BYTES: u64 = 64 * 1024 * 1024;
fn read(path: &Path) -> crate::Result<Vec<u8>> {
    let metadata = std::fs::metadata(path).map_err(error)?;
    if metadata.len() > MAX_BYTES {
        return Err(error("mesh resource exceeds 64 MiB"));
    }
    std::fs::read(path).map_err(error)
}
fn resource(name: String, role: &str, bytes: &[u8]) -> Resource {
    Resource {
        name,
        role: role.into(),
        sha256: Some(format!("{:x}", Sha256::digest(bytes))),
        license: None,
    }
}
fn local_buffer(root: &Path, uri: &str) -> crate::Result<Vec<u8>> {
    if uri.starts_with("data:") {
        let (header, encoded) = uri
            .split_once(',')
            .ok_or_else(|| error("invalid buffer data URI"))?;
        if !matches!(
            header,
            "data:application/octet-stream;base64" | "data:application/gltf-buffer;base64"
        ) {
            return Err(error("only base64 binary buffer data URIs are supported"));
        }
        return base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(error);
    }
    // Reject URL escapes and absolute/parent paths; no asset can pull resources
    // outside its supplied directory, including through a symlink.
    if uri.is_empty()
        || uri.contains([':', '%', '\\'])
        || Path::new(uri)
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(error(
            "buffer URI must be a local relative path without parent components or URL escapes",
        ));
    }
    let root = std::fs::canonicalize(root).map_err(error)?;
    let path = std::fs::canonicalize(root.join(uri)).map_err(error)?;
    if !path.starts_with(&root) {
        return Err(error("buffer resolves outside mesh directory"));
    }
    read(&path)
}
fn obj(bytes: &[u8], mesh: &mut Mesh) -> crate::Result<()> {
    let options = tobj::LoadOptions {
        triangulate: false,
        single_index: false,
        ignore_points: false,
        ignore_lines: false,
    };
    let (models, _) = tobj::load_obj_buf(&mut std::io::Cursor::new(bytes), &options, |_| {
        Ok(Default::default())
    })
    .map_err(error)?;
    for model in models {
        let m = model.mesh;
        if m.face_arities.iter().any(|&n| n != 3) || !m.indices.len().is_multiple_of(3) {
            return Err(error(
                "OBJ supports triangle faces only; triangulate polygons explicitly before measurement",
            ));
        }
        let offset = u32::try_from(mesh.vertices.len()).map_err(error)?;
        mesh.vertices.extend(
            m.positions
                .as_chunks::<3>()
                .0
                .iter()
                .map(|v| Point3::new(v[0], v[1], v[2])),
        );
        for t in m.indices.as_chunks::<3>().0 {
            mesh.triangles
                .push([t[0] + offset, t[1] + offset, t[2] + offset]);
        }
    }
    mesh.limitations.push("OBJ has no declared unit; --unit is the caller's declaration. Only f64 positions and triangle indices are decoded; MTL, UVs, colours and shading normals are outside the identity proof.".into());
    Ok(())
}
fn numbers<const N: usize>(
    value: Option<&serde_json::Value>,
    default: [f64; N],
) -> crate::Result<[f64; N]> {
    let Some(value) = value else {
        return Ok(default);
    };
    let array = value
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or_else(|| error("invalid node transform dimensions"))?;
    let mut out = default;
    for (dst, src) in out.iter_mut().zip(array) {
        *dst = src
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| error("nonfinite node transform"))?;
    }
    Ok(out)
}
fn transform(raw: &serde_json::Value) -> crate::Result<Matrix4<f64>> {
    if let Some(matrix) = raw.get("matrix") {
        let m = Matrix4::from_column_slice(&numbers(Some(matrix), [0.0; 16])?);
        if m[(3, 0)] != 0.0 || m[(3, 1)] != 0.0 || m[(3, 2)] != 0.0 || m[(3, 3)] != 1.0 {
            return Err(error("node matrix must be affine"));
        }
        return Ok(m);
    }
    let t = numbers(raw.get("translation"), [0.0; 3])?;
    let s = numbers(raw.get("scale"), [1.0; 3])?;
    let r = numbers(raw.get("rotation"), [0.0, 0.0, 0.0, 1.0])?;
    let q = Quaternion::new(r[3], r[0], r[1], r[2]);
    if (q.norm() - 1.0).abs() > 1e-6 {
        return Err(error("node quaternion must have unit length"));
    }
    Ok(Matrix4::new_translation(&Vector3::from(t))
        * UnitQuaternion::new_normalize(q).to_homogeneous()
        * Matrix4::new_nonuniform_scaling(&Vector3::from(s)))
}
fn visit(
    node: gltf::Node<'_>,
    parent: Matrix4<f64>,
    raw: &serde_json::Value,
    buffers: &[Vec<u8>],
    mesh: &mut Mesh,
    stack: &mut Vec<usize>,
) -> crate::Result<()> {
    if stack.len() >= 256 || stack.contains(&node.index()) {
        return Err(error("cyclic or excessive node hierarchy"));
    }
    if node.skin().is_some() || node.weights().is_some() {
        return Err(error(
            "skinning and morph weights are unsupported; bake static geometry first",
        ));
    }
    stack.push(node.index());
    // gltf's convenience transform is f32. Read JSON transforms as f64 instead,
    // preserving large translations and small world-space deltas.
    let world = parent * transform(&raw["nodes"][node.index()])?;
    if let Some(object) = node.mesh() {
        if object.weights().is_some() {
            return Err(error("mesh morph weights are unsupported"));
        }
        for primitive in object.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles
                || primitive.morph_targets().next().is_some()
            {
                return Err(error("only static triangle primitives are supported"));
            }
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
            let positions = reader
                .read_positions()
                .ok_or_else(|| error("missing or invalid POSITION accessor"))?;
            let offset = u32::try_from(mesh.vertices.len()).map_err(error)?;
            let mut count = 0u32;
            for p in positions {
                mesh.vertices.push(world.transform_point(&Point3::new(
                    f64::from(p[0]),
                    f64::from(p[1]),
                    f64::from(p[2]),
                )));
                count += 1;
            }
            let indices: Vec<u32> = reader
                .read_indices()
                .map_or_else(|| (0..count).collect(), |i| i.into_u32().collect());
            if !indices.len().is_multiple_of(3) || indices.iter().any(|&i| i >= count) {
                return Err(error("invalid primitive triangle indices"));
            }
            for t in indices.as_chunks::<3>().0 {
                mesh.triangles
                    .push([offset + t[0], offset + t[1], offset + t[2]]);
            }
        }
    }
    for child in node.children() {
        visit(child, world, raw, buffers, mesh, stack)?;
    }
    stack.pop();
    Ok(())
}
fn gltf(path: &Path, bytes: &[u8], mesh: &mut Mesh) -> crate::Result<()> {
    let parsed = gltf::Gltf::from_slice(bytes).map_err(error)?;
    if parsed.extensions_required().next().is_some() {
        return Err(error("required glTF extensions are unsupported"));
    }
    if parsed.animations().next().is_some() {
        return Err(error(
            "animated glTF is unsupported; bake a static pose first",
        ));
    }
    let raw: serde_json::Value = if bytes.starts_with(b"glTF") {
        serde_json::from_slice(&gltf::binary::Glb::from_slice(bytes).map_err(error)?.json)
            .map_err(error)?
    } else {
        serde_json::from_slice(bytes).map_err(error)?
    };
    let mut buffers = Vec::new();
    let mut total = bytes.len();
    for buffer in parsed.buffers() {
        let data = match buffer.source() {
            gltf::buffer::Source::Bin => parsed
                .blob
                .clone()
                .ok_or_else(|| error("GLB binary chunk missing"))?,
            gltf::buffer::Source::Uri(uri) => local_buffer(
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new(".")),
                uri,
            )?,
        };
        total = total
            .checked_add(data.len())
            .ok_or_else(|| error("buffer size overflow"))?;
        if total > MAX_BYTES as usize || data.len() < buffer.length() {
            return Err(error(
                "incomplete buffers or aggregate resource size exceeds 64 MiB",
            ));
        }
        mesh.resources.push(resource(
            format!("buffer:{}", buffer.index()),
            "geometry_buffer",
            &data,
        ));
        buffers.push(data);
    }
    let scene = parsed
        .default_scene()
        .or_else(|| {
            if parsed.scenes().count() == 1 {
                parsed.scenes().next()
            } else {
                None
            }
        })
        .ok_or_else(|| error("glTF needs a default scene or one unambiguous scene"))?;
    mesh.limitations.push(format!("glTF scene {} only; f32 POSITION samples promoted exactly to f64, JSON node transforms applied in f64. glTF coordinates conventionally use metres; --unit must describe the supplied coordinates. Images, materials and other appearance attributes are not decoded or proven identical.",scene.index()));
    for node in scene.nodes() {
        visit(
            node,
            Matrix4::identity(),
            &raw,
            &buffers,
            mesh,
            &mut Vec::new(),
        )?;
    }
    Ok(())
}
/// Reads a static triangle OBJ, glTF or GLB; rejects unsupported or invalid geometry.
pub fn load(path: &Path) -> crate::Result<Mesh> {
    let bytes = read(path)?;
    let mut mesh = Mesh {
        vertices: vec![],
        triangles: vec![],
        resources: vec![resource("document".into(), "mesh_document", &bytes)],
        limitations: vec![],
    };
    match path
        .extension()
        .and_then(|x| x.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("obj") => obj(&bytes, &mut mesh)?,
        Some("gltf" | "glb") => gltf(path, &bytes, &mut mesh)?,
        _ => return Err(error("input must be .obj, .gltf or .glb")),
    }
    mesh.validate()?;
    Ok(mesh)
}
