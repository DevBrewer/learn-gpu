use std::path::{Path, PathBuf};

use wgpu::util::DeviceExt;

use crate::{model, texture};

/// Resolve a file path relative to the root `assets` folder.
pub fn get_asset_path(file_name: &str) -> PathBuf {
    // Check if relative path from current working directory exists
    let candidates = [
        Path::new("assets").join(file_name),
        Path::new("../../assets").join(file_name),
        Path::new("../../../assets").join(file_name),
    ];

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| {
            // Fallback using compile-time manifest dir:
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets")
                .join(file_name)
        })
}

/// Load a UTF-8 text string from the assets directory asynchronously.
#[allow(dead_code)]
pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    let path = get_asset_path(file_name);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read assest text from {path:?}: {e}"))?;

    Ok(text)
}

/// Loads raw binary bytes from the assets directory asynchronously.
pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let path = get_asset_path(file_name);
    let data = std::fs::read(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read asset binary from {path:?}: {e}"))?;

    Ok(data)
}

/// Load an image from the assets directory into a GPU Texture.
pub async fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name)
}

/// Loads a glTF model (.gltf + .bin) using the project asset directory resolver.
pub async fn load_gltf_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    // 1. Resolve path and import .gltf + associated .bin file simultaneously
    let gltf_path = get_asset_path(file_name);
    let (document, buffers, _) = gltf::import(&gltf_path)
        .map_err(|e| anyhow::anyhow!("Failed to import glTF from {gltf_path:?}: {e}"))?;

    // 2. Process Materials and compile Bind Groups
    let mut materials = Vec::new();
    for mat in document.materials() {
        // Extract texture name from PBR properties, fallback if undefined
        let texture_file = mat
            .pbr_metallic_roughness()
            .base_color_texture()
            .and_then(|t| match t.texture().source().source() {
                gltf::image::Source::Uri { uri, .. } => Some(uri),
                _ => None,
            })
            .unwrap_or("cube-diffuse.jpg");

        let diffuse_texture = load_texture(texture_file, device, queue).await?;

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("Material BindGroup: {:?}", mat.name())),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
        });

        materials.push(model::Material {
            name: mat.name().unwrap_or("Material").to_string(),
            diffuse_texture,
            bind_group,
        });
    }

    // 3. Extract Mesh primitives and load into GPU buffers
    let mut meshes = Vec::new();
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));

            // Extract vertex attributes seamlessly
            let positions = reader
                .read_positions()
                .ok_or_else(|| anyhow::anyhow!("Mesh missing positions"))?
                .collect::<Vec<_>>();

            let mut tex_coords = vec![[0.0, 0.0]; positions.len()];
            if let Some(uv_iter) = reader.read_tex_coords(0) {
                for (i, uv) in uv_iter.into_f32().enumerate() {
                    tex_coords[i] = [uv[0], 1.0 - uv[1]]; // Correct inverted glTF V-axis on the fly
                }
            }

            let mut normals = vec![[0.0, 1.0, 0.0]; positions.len()];
            if let Some(normal_iter) = reader.read_normals() {
                for (i, normal) in normal_iter.enumerate() {
                    normals[i] = normal;
                }
            }

            // Interleave attributes into pipeline vertex layouts
            let vertices = (0..positions.len())
                .map(|i| model::ModelVertex {
                    position: positions[i],
                    tex_coords: tex_coords[i],
                    normal: normals[i],
                })
                .collect::<Vec<_>>();

            let indices = reader
                .read_indices()
                .ok_or_else(|| anyhow::anyhow!("Mesh missing indices"))?
                .into_u32()
                .collect::<Vec<_>>();

            // Allocate GPU buffers
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
