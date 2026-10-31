# pgt

A programmable GPU toolkit providing modular building blocks around `wgpu`, `winit`, and `egui` to eliminate repetitive graphics and UI boilerplate.

## What is in here

- `core`: Window runner and input state tracking (`run`, `App`, `Context`, `InputState`).
- `buffer`: Type-safe wrappers for uniform and storage buffers (`UniformBinding`, `StorageBinding`).
- `pipeline`: Fluent `PipelineBuilder` for declarative `RenderPipeline` creation.
- `camera`: Orbit (turntable) and free-look camera controllers (`OrbitCamera`, `FlyCamera`, `CameraUniform`).
- `texture`: 2D textures, samplers, and depth attachments (`Texture2D`, `DepthTexture`, `Samplers`).
- `model`: glTF 2.0 loader with automatic tangent/bitangent calculation (`Model`, `Mesh`, `load_gltf`).
- `ui`: Zero-boilerplate `egui` overlay pass giving direct access to root `&mut egui::Ui`.

## Running the demo

```bash
cargo run -p demo-pgt
```

## Known issues & missing parts

- Abstractions are rigid and assume specific bind group layouts and pass structures.
- Lighting in the demo is basic Blinn-Phong rather than a full PBR metallic-roughness setup.
- Model loading only supports static meshes with basic diffuse/normal textures; no skinning, animations, or advanced materials.
- Error handling is rudimentary.
