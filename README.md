# 🚀 Learn GPU — Rust & WGPU Graphics Programming

A repository dedicated to exploring modern GPU programming and real-time graphics rendering in **Rust** using **`wgpu`** and **`winit`**.

This project follows the popular tutorial series from [**Learn Wgpu (sotrh.github.io/learn-wgpu)**](https://sotrh.github.io/learn-wgpu/), adapted with modern Rust ecosystem choices and custom abstractions:
- **Math Library**: Uses [`glam`](https://crates.io/crates/glam) instead of `cgmath`.
- **Framework Abstraction**: Includes a custom shared `framework` crate instead of repeating boilerplate event-loop/surface initialization in each tutorial.
- **Model Loading**: Supports GLTF/OBJ formats with custom vertex descriptors and texture loading pipelines.

---

## 🎨 Overview & Workspace Architecture

`learn-gpu` is organized as a Cargo workspace with step-by-step interactive chapters and a shared helper crate (`framework`).

### 📦 Key Components

1. **`framework` Crate** (`framework/`)
   - Manages the `winit` 0.30 `ApplicationHandler` lifecycle.
   - Provides an `App` trait defining `init`, `update`, `render`, `resize`, and `handle_key` hooks.
   - Manages device, queue, surface configuration, and context setup.

2. **Interactive Chapters** (`chapters/`)
   - Each tutorial is an independent crate organized in incremental difficulty.

---

## 🎮 Tutorial Chapters

| Chapter | Description | Key Topics / Crates |
| :--- | :--- | :--- |
| **Tutorial 1: Window** | Basic window creation and event loop handling | `winit` 0.30, `ApplicationHandler` |
| **Tutorial 2: Triangle** | First draw pass, render pipeline, and vertex shaders | `wgpu`, WGSL, swapchain clear color |
| **Tutorial 3: Buffers** | Vertex and index buffers | Vertex layouts, indexed rendering |
| **Tutorial 4: Textures** | Texture loading, sampling, and bind groups | `image`, `wgpu::Texture`, `wgpu::Sampler` |
| **Tutorial 5: Uniforms** | 3D Camera, uniform buffers, and view-projection matrices | `glam`, perspective projection, uniform buffers |
| **Tutorial 6: Instance** | Hardware instancing for rendering multiple objects | Instance buffer, quaternions, instanced draw calls |
| **Tutorial 7: Particles** | Dynamic particle updates and instanced compute/vertex rendering | Instancing, per-frame buffer updates |
| **Tutorial 8: Model** | 3D model loading (GLTF) and multi-material meshes | `gltf`, mesh indices, multiple bind groups |
| **Tutorial 9: Light** | Blinn-Phong lighting model with dynamic orbiting point light | Normal vectors, light uniforms, custom light shader pass |
| **Tutorial 10: Normals** | Tangent-space normal mapping with bump maps | Tangent & bitangent generation, normal maps, TBN matrix in WGSL |
| **Tutorial 11: Better Camera** | First-person camera controller with yaw/pitch and projection | Decoupled `Camera`, `Projection`, mouse look & wheel zoom |

---

## ⚙️ Key Technical Differences from the Original Tutorial

While following [Learn Wgpu](https://sotrh.github.io/learn-wgpu/), this repository modernizes and adapts key crates and techniques:

### 📦 Crate Comparisons

| Role / Purpose | Original Tutorial Crate | This Repository | Why the Change? |
| :--- | :--- | :--- | :--- |
| **Math Library** | [`cgmath`](https://crates.io/crates/cgmath) | [`glam`](https://crates.io/crates/glam) | Fast, SIMD-aligned, ergonomics-friendly, modern standard in Rust game dev & WebGPU (with direct DirectX/WebGPU NDC support). |
| **3D Model Loading** | [`tobj`](https://crates.io/crates/tobj) (`.obj`) | [`gltf`](https://crates.io/crates/gltf) (`.gltf` / `.bin`) | Industry standard glTF 2.0 asset workflow with PBR material definitions, modern scene hierarchies, and binary buffers. |
| **Windowing & Events** | `winit` 0.28/0.29 (legacy `event_loop.run`) | `winit` 0.30 (`ApplicationHandler`) | Upgraded to the modern event-driven `ApplicationHandler` API required in `winit` 0.30+. |
| **Async Executor** | `futures` block_on | [`pollster`](https://crates.io/crates/pollster) | Minimal, zero-overhead blocking executor designed specifically for WebGPU device initialization without heavy async runtimes. |
| **Texture Loading** | `image` (legacy formats) | [`image`](https://crates.io/crates/image) (v0.25+) | Configured with sRGB for color textures (`Rgba8UnormSrgb`) and Linear for normal maps (`Rgba8Unorm`). |
| **Boilerplate Handling**| Repeated `main.rs` setup per tutorial | Custom `framework` crate | Clean lifecycle abstraction (`App` trait) reducing repetitive event loop and swapchain setup. |

### 🛠 Math & Shader Details
1. **`glam` Projection**:
   - Camera view-projection utilizes `glam::Mat4::perspective_rh` and coordinates configured for WebGPU's $[0, 1]$ depth clip space.
   - Rotations and orbital light physics use `glam::Quat` and `glam::Vec3`.
2. **Tangent & Bitangent Generation**:
   - Because glTF files often omit explicit tangent vectors (or store 4D tangents without bitangents), [resources.rs](chapters/tutorial10-normals/src/resources.rs) computes triangle face tangents and bitangents from UVs and positions on-the-fly and averages them per vertex.
3. **WGSL Alignment Rules**:
   - Uniforms like `LightUniform` strictly adhere to WebGPU 16-byte alignment rules (`vec3<f32>` padded with `_padding: u32`).

---

## 🚀 Running the Tutorials

Run any tutorial crate directly using Cargo:

```bash
# Run Tutorial 1: Window
cargo run -p tutorial1-window

# Run Tutorial 2: Triangle
cargo run -p tutorial2-triangle

# Run Tutorial 3: Buffers
cargo run -p tutorial3-buffers

# Run Tutorial 4: Textures
cargo run -p tutorial4-Textures

# Run Tutorial 5: Uniforms & 3D Camera
cargo run -p tutorial5-uniforms

# Run Tutorial 6: Instancing
cargo run -p tutorial6-instance

# Run Tutorial 7: Particles
cargo run -p tutorial7-particles

# Run Tutorial 8: 3D Model Loading
cargo run -p tutorial8-model

# Run Tutorial 9: Lighting
cargo run -p tutorial9-light

# Run Tutorial 10: Normal Mapping
cargo run -p tutorial10-normals

# Run Tutorial 11: A Better Camera
cargo run -p tutorial11-better-camera
```

### Controls (Tutorial 11: Better Camera)
- **`W` / `Up`**: Move camera forward (along horizontal facing direction)
- **`S` / `Down`**: Move camera backward
- **`A` / `Left`**: Strafe camera left
- **`D` / `Right`**: Strafe camera right
- **`Space`**: Fly up
- **`Shift`**: Fly down
- **Left Mouse Click + Drag**: Free-look rotate (yaw & pitch)
- **Mouse Wheel Scroll**: Zoom / dolly along view vector
- **`Esc`**: Exit application

---

## 🛠 Tech Stack

- **Language**: Rust (Edition 2024)
- **Graphics API**: `wgpu` (30.0)
- **Math Library**: `glam` (0.34)
- **Windowing**: `winit` (0.30)
- **Asset Loading**: `gltf` (1.4), `image` (0.25)
- **Tutorial Reference**: [Learn Wgpu by sotrh](https://sotrh.github.io/learn-wgpu/)
