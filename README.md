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
| **Tutorial 8: Model** | 3D model loading (GLTF / OBJ) and multi-material meshes | `gltf`, mesh indices, multiple bind groups |
| **Tutorial 9: Light** | Blinn-Phong lighting model with dynamic orbiting point light | Normal vectors, light uniforms, custom light shader pass |

---

## ⚙️ Key Technical Differences from the Original Tutorial

While following [Learn Wgpu](https://sotrh.github.io/learn-wgpu/), this repository introduces notable differences:

1. **`glam` for Math**:
   - Camera view-projection utilizes `glam::camera::rh::proj::directx::perspective` to ensure proper right-handed, Y-up clip space mapping matching WebGPU conventions.
   - Instance rotations and light orbital mechanics utilize `glam::Quat` and `glam::Vec3`.
2. **WGSL Alignment Rules**:
   - `LightUniform` adheres to WebGPU 16-byte alignment (`vec3<f32>` padded with `_padding: u32`).
3. **Modular Submodule Architecture**:
   - Each chapter is isolated with its own Git history and dependencies managed under a shared top-level workspace.

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
```

### Controls (Tutorial 9: Light)
- **`W` / `Up`**: Move camera forward
- **`S` / `Down`**: Move camera backward
- **`A` / `Left`**: Orbit camera left
- **`D` / `Right`**: Orbit camera right
- **`Space`**: Move camera up
- **`Shift`**: Move camera down
- **`Esc`**: Exit application

---

## 🛠 Tech Stack

- **Language**: Rust (Edition 2024)
- **Graphics API**: `wgpu` (30.0)
- **Math Library**: `glam` (0.34)
- **Windowing**: `winit` (0.30)
- **Asset Loading**: `gltf` (1.4), `image` (0.25)
- **Tutorial Reference**: [Learn Wgpu by sotrh](https://sotrh.github.io/learn-wgpu/)
