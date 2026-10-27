#![allow(unused)]

use std::path::{Path, PathBuf};

use wgpu::util::DeviceExt;

use crate::model;
use crate::model::ModelVertex;
use crate::texture;

/// Resolve a file path relative to the root `assests` folder.
pub fn get_assets_path(file_name: &str) -> PathBuf {
    // Check if relative path from current working directory exists
    let candidates = [
        Path::new("assets").join(file_name),
        Path::new("../../assests").join(file_name),
        Path::new("../../../assets").join(file_name),
    ];

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| {
            // FallBack using compile-time manifest dir:
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets")
                .join(file_name)
        })
}

/// Load a UTF-8 text string from the assets directory asynchronously.
pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    let path = get_assets_path(file_name);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read text from {path:?}: {e}"))?;

    Ok(text)
}

/// Load raw binary bytes from the assets directory asynchronously.
pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let path = get_assets_path(file_name);
    let data = std::fs::read(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read binary data from {path:?}: {e}"))?;

    Ok(data)
}

/// Load an image from the assests directory into a GPU Texture.
pub async fn load_texture(
    file_name: &str,
    is_normal_map: bool,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name, is_normal_map)
}

/// Loads a glTF model (.glTF + .bin) using the project asset directory resovler.
pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    let path = get_assets_path(file_name);
    let (document, buffers, _) = gltf::import(&path)
        .map_err(|e| anyhow::anyhow!("Failed to load model from {path:?}: {e}"))?;

    // Materials
    let mut materials = Vec::new();
    for mat in document.materials() {
        let texture_file = mat
            .pbr_metallic_roughness()
            .base_color_texture()
            .and_then(|t| match t.texture().source().source() {
                gltf::image::Source::Uri { uri, .. } => Some(uri),
                _ => None,
            })
            .unwrap_or("cube-diffuse.jpg");

        let diffuse_texture = load_texture(texture_file, false, device, queue).await?;

        let normal_file = mat
            .normal_texture()
            .and_then(|t| match t.texture().source().source() {
                gltf::image::Source::Uri { uri, .. } => Some(uri),
                _ => None,
            })
            .unwrap_or("cube-normal.png");

        let normal_texture = load_texture(normal_file, true, device, queue).await?;

        materials.push(model::Material::new(
            device,
            mat.name().unwrap_or("Material"),
            diffuse_texture,
            normal_texture,
            layout,
        ));
    }

    // Meshes
    let mut meshes = Vec::new();
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));

            // Extract positions
            let positons = reader
                .read_positions()
                .ok_or_else(|| anyhow::anyhow!("Mesh missing positions"))?
                .collect::<Vec<_>>();

            let count = positons.len();

            let mut tex_coords_iter = reader.read_tex_coords(0).map(|it| it.into_f32());
            let mut normals_iter = reader.read_normals();

            let mut vertices = (0..count)
                .map(|i| {
                    let position = positons[i];

                    let tex_coords = tex_coords_iter
                        .as_mut()
                        .and_then(|it| it.next())
                        .map(|uv| [uv[0], 1.0 - uv[1]])
                        .unwrap_or([0.0, 0.0]);

                    let normal = normals_iter
                        .as_mut()
                        .and_then(|it| it.next())
                        .unwrap_or([0.0, 1.0, 0.0]);

                    model::ModelVertex {
                        position,
                        tex_coords,
                        normal,
                        tangent: [0.0; 3],
                        bitangent: [0.0; 3],
                    }
                })
                .collect::<Vec<_>>();

            // 4. extract indices
            let indices = reader
                .read_indices()
                .ok_or_else(|| anyhow::anyhow!("Mesh missing indices"))?
                .into_u32()
                .collect::<Vec<_>>();

            // Calculate tangents and bitangents per triangle face
            let mut triangles_included = vec![0; vertices.len()];
            for chunk in indices.chunks(3) {
                if chunk.len() < 3 {
                    continue;
                }
                let i0 = chunk[0] as usize;
                let i1 = chunk[1] as usize;
                let i2 = chunk[2] as usize;

                let v0 = vertices[i0];
                let v1 = vertices[i1];
                let v2 = vertices[i2];

                // A triangle 3D world space position
                let pos0 = glam::Vec3::from_slice(&v0.position);
                let pos1 = glam::Vec3::from_slice(&v1.position);
                let pos2 = glam::Vec3::from_slice(&v2.position);

                // Texture space in 2D UV
                let uv0 = glam::Vec2::from_slice(&v0.tex_coords);
                let uv1 = glam::Vec2::from_slice(&v1.tex_coords);
                let uv2 = glam::Vec2::from_slice(&v2.tex_coords);

                // Calculate the triangle edges
                let delta_pos1 = pos1 - pos0;
                let delta_pos2 = pos2 - pos0;

                // This will give a direction to calculate the
                // tangent and bitgangent
                let delta_uv1 = uv1 - uv0;
                let delta_uv2 = uv2 - uv0;

                /*
                 * ΔP1 = Δu1 * T + Δv1 * B
                 * ΔP2 = Δu2 * T + Δv2 * B
                 *
                 * In matrix form:
                 * ┌   ┐   ┌        ┐┌   ┐
                 * │ΔP1│ = │Δu1  Δv1││ T │
                 * │ΔP2│   │Δu2  Δv2││ B │
                 * └   ┘   └        ┘└   ┘
                 *
                 * Inverting that 2×2 matrix gives:
                 * ┌   ┐          1         ┌          ┐┌   ┐
                 * │ T │ = ───────────────  │ Δv2 -Δv1 ││ΔP1│
                 * │ B │   Δu1Δv2 - Δv1Δu2  │-Δu2  Δu1 ││ΔP2│
                 * └   ┘                    └          ┘└   ┘
                 */

                // Determinant reciprocal:
                let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let r = if r.is_finite() { r } else { 0.0 };

                // T = (ΔP1 * Δv2 - ΔP2 * Δv1) * r
                let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;

                // B = (ΔP2 * Δu1 - ΔP1 * Δu2) * r
                let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * r;

                for &idx in &[i0, i1, i2] {
                    vertices[idx].tangent =
                        (glam::Vec3::from_slice(&vertices[idx].tangent) + tangent).to_array();
                    vertices[idx].bitangent =
                        (glam::Vec3::from_slice(&vertices[idx].bitangent) + bitangent).to_array();
                    triangles_included[idx] += 1;
                }
            }

            // Average and normalize the tangents and bitangents
            for (i, v) in vertices.iter_mut().enumerate() {
                let count = triangles_included[i].max(1) as f32;
                let tan = glam::Vec3::from_slice(&v.tangent) / count;
                let bitan = glam::Vec3::from_slice(&v.bitangent) / count;
                v.tangent = tan.normalize_or_zero().to_array();
                v.bitangent = bitan.normalize_or_zero().to_array();
            }

            // Allocate GPU Buffers
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?}_vertex", mesh.name())),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?}_index", mesh.name())),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            meshes.push(model::Mesh {
                name: mesh.name().unwrap_or("Mesh").to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: indices.len() as u32,
                material: primitive.material().index().unwrap_or(0),
            });
        }
    }
    Ok(model::Model { meshes, materials })
}
