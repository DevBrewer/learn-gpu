use std::path::Path;
use wgpu::util::DeviceExt;
use crate::texture::Texture2D;
use super::mesh::{Material, Mesh, Model};
use super::vertex::ModelVertex;

/// Load a 3D model from a glTF (.glb or .gltf) file.
///
/// Automatically parses meshes, indices, texture materials, and computes
/// tangent and bitangent vectors on the fly if missing.
pub fn load_gltf(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: impl AsRef<Path>,
) -> anyhow::Result<Model> {
    let path = path.as_ref();
    let (document, buffers, images) = gltf::import(path)?;

    let material_layout = Material::create_layout(device);
    let mut materials = Vec::new();

    // 1. Process materials & textures
    for mat in document.materials() {
        let pbr = mat.pbr_metallic_roughness();

        // Diffuse / Base color
        let diffuse_texture = if let Some(info) = pbr.base_color_texture() {
            let img = &images[info.texture().source().index()];
            create_texture_from_gltf_image(device, queue, img, "BaseColor", true)?
        } else {
            Texture2D::create_white_pixel(device, queue)
        };

        // Normal map
        let normal_texture = if let Some(info) = mat.normal_texture() {
            let img = &images[info.texture().source().index()];
            create_texture_from_gltf_image(device, queue, img, "Normal", false)?
        } else {
            // Default flat normal [0.5, 0.5, 1.0, 1.0] representing pointing along +Z in tangent space
            create_flat_normal_texture(device, queue)
        };

        materials.push(Material::new(
            device,
            mat.name().unwrap_or("Material"),
            diffuse_texture,
            normal_texture,
            &material_layout,
        ));
    }

    if materials.is_empty() {
        materials.push(Material::new(
            device,
            "DefaultMaterial",
            Texture2D::create_white_pixel(device, queue),
            create_flat_normal_texture(device, queue),
            &material_layout,
        ));
    }

    // 2. Process meshes and calculate tangents/bitangents
    let mut meshes = Vec::new();

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let positions = reader.read_positions().map(|p| p.collect::<Vec<_>>()).unwrap_or_default();
            let tex_coords = reader
                .read_tex_coords(0)
                .map(|tc| tc.into_f32().collect::<Vec<_>>())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
            let normals = reader.read_normals().map(|n| n.collect::<Vec<_>>()).unwrap_or_default();

            let mut vertices = Vec::with_capacity(positions.len());
            for (i, &pos) in positions.iter().enumerate() {
                vertices.push(ModelVertex {
                    position: pos,
                    tex_coords: tex_coords.get(i).copied().unwrap_or([0.0, 0.0]),
                    normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
                    tangent: [0.0; 3],
                    bitangent: [0.0; 3],
                });
            }

            let indices = reader.read_indices().map(|idx| idx.into_u32().collect::<Vec<_>>()).unwrap_or_default();

            // Calculate tangents & bitangents per triangle face
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

                let pos0 = glam::Vec3::from_slice(&v0.position);
                let pos1 = glam::Vec3::from_slice(&v1.position);
                let pos2 = glam::Vec3::from_slice(&v2.position);

                let uv0 = glam::Vec2::from_slice(&v0.tex_coords);
                let uv1 = glam::Vec2::from_slice(&v1.tex_coords);
                let uv2 = glam::Vec2::from_slice(&v2.tex_coords);

                let delta_pos1 = pos1 - pos0;
                let delta_pos2 = pos2 - pos0;
                let delta_uv1 = uv1 - uv0;
                let delta_uv2 = uv2 - uv0;

                let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let r = if r.is_finite() { r } else { 0.0 };

                let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
                let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * r;

                for &idx in &[i0, i1, i2] {
                    vertices[idx].tangent =
                        (glam::Vec3::from_slice(&vertices[idx].tangent) + tangent).to_array();
                    vertices[idx].bitangent =
                        (glam::Vec3::from_slice(&vertices[idx].bitangent) + bitangent).to_array();
                    triangles_included[idx] += 1;
                }
            }

            // Average and normalize per-vertex tangents
            for (i, v) in vertices.iter_mut().enumerate() {
                let count = triangles_included[i].max(1) as f32;
                let tan = glam::Vec3::from_slice(&v.tangent) / count;
                let bitan = glam::Vec3::from_slice(&v.bitangent) / count;
                v.tangent = tan.normalize_or_zero().to_array();
                v.bitangent = bitan.normalize_or_zero().to_array();
            }

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{}_vertex", mesh.name().unwrap_or("mesh"))),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{}_index", mesh.name().unwrap_or("mesh"))),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            meshes.push(Mesh {
                name: mesh.name().unwrap_or("mesh").to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: indices.len() as u32,
                material_index: primitive.material().index().unwrap_or(0),
            });
        }
    }

    Ok(Model { meshes, materials })
}

fn create_texture_from_gltf_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    image_data: &gltf::image::Data,
    label: &str,
    is_srgb: bool,
) -> anyhow::Result<Texture2D> {
    use image::RgbaImage;
    let rgba_buffer = match image_data.format {
        gltf::image::Format::R8G8B8 => {
            let mut rgba = Vec::with_capacity(image_data.width as usize * image_data.height as usize * 4);
            for chunk in image_data.pixels.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            rgba
        }
        gltf::image::Format::R8G8B8A8 => image_data.pixels.clone(),
        _ => {
            let img = image::load_from_memory(&image_data.pixels)?;
            img.to_rgba8().into_raw()
        }
    };

    let img = RgbaImage::from_raw(image_data.width, image_data.height, rgba_buffer)
        .ok_or_else(|| anyhow::anyhow!("Failed to construct RgbaImage from glTF image buffer"))?;

    Ok(Texture2D::from_image(device, queue, &image::DynamicImage::ImageRgba8(img), label, is_srgb))
}

fn create_flat_normal_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> Texture2D {
    use crate::texture::Samplers;
    let size = wgpu::Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Default Flat Normal Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // RGBA = [128, 128, 255, 255] corresponds to tangent-space normal [0.0, 0.0, 1.0]
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[128, 128, 255, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = Samplers::linear_clamp(device);

    Texture2D {
        texture,
        view,
        sampler,
        width: 1,
        height: 1,
        format: wgpu::TextureFormat::Rgba8Unorm,
    }
}
