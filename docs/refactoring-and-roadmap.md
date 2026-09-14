# Nova Engine — Refactoring Priorities

> This document lists high-priority refactoring tasks that must be completed
> before further editor development, plus detailed steps for each planned
> roadmap item. Tasks are ordered by execution priority.

---

## High Priority — wgpu/winit Encapsulation Refactors

### R1. Engine-native `GraphicsConfiguration` enums

**Problem**: `GraphicsConfiguration` exposes raw `wgpu` types to application code:

```rust
// current — leaks wgpu to apps
pub struct GraphicsConfiguration {
    pub power_preference: wgpu::PowerPreference,
    pub present_mode: wgpu::PresentMode,
    pub alpha_mode: wgpu::CompositeAlphaMode,
}
```

Applications calling `.with_power_preference(wgpu::PowerPreference::LowPower)` depend on `wgpu` directly — violating the boundary rule.

**Tasks**:

- [x] Create engine-native enums in `nova-core/src/graphics/config.rs`:
  - `PowerPreference { LowPower, HighPerformance, None }`
  - `PresentMode { AutoVsync, AutoNoVsync, Mailbox, Fifo, FifoRelaxed }`
  - `CompositeAlphaMode { Auto, Opaque, PreMultiplied, PostMultiplied, Inherit }`
- [x] Implement `From<EngineEnum> for wgpu::Enum` for each.
- [x] Update `GraphicsConfiguration` fields to use the engine-native enums.
- [x] Update builder methods (`with_power_preference`, `with_present_mode`, `with_alpha_mode`) to take engine-native types.
- [x] Update `GraphicsContext::create_surface_config` to convert via `Into`.
- [x] Update all call sites (nova-test, nova-editor) to use engine-native enums.

**Files affected**:
- `nova-core/src/graphics/config.rs` — new enums + updated struct
- `nova-core/src/graphics/context.rs` — use `.into()` when constructing wgpu config
- `nova-test/src/main.rs` — update if using graphics config builders
- `nova-editor/src/main.rs` — same

---

### R2. Engine-native `ControlFlow` enum

**Problem**: `ApplicationBuilder::with_control_flow` takes `winit::event_loop::ControlFlow`, leaking winit to apps.

**Tasks**:

- [x] Create `nova-core/src/app.rs` (or a new `nova-core/src/app/control_flow.rs`:
  ```rust
  pub enum ControlFlow {
      Poll,
      Wait,
      WaitUntil(Duration),
  }
  ```
- [x] Implement `From<ControlFlow> for winit::event_loop::ControlFlow`.
- [x] Update `ApplicationBuilder::with_control_flow` to take the engine-native type.
- [x] Update `Application` struct field and handler logic to convert at the boundary.
- [x] Update all call sites.

**Files affected**:
- `nova-core/src/app.rs` or `nova-core/src/app/control_flow.rs` — new enum
- `nova-core/src/app/builder.rs` — update import + method signature
- `nova-core/src/app/handler.rs` — convert at the winit boundary

---

### R3. Engine-native `WindowAttributes`

**Problem**: `ApplicationBuilder::alter_window_attributes` exposes `winit::window::WindowAttributes` to applications. Apps must know winit's API (`with_inner_size`, `with_title`, etc.) and winit types (`LogicalSize`, etc.).

**Tasks**:

- [x] Create `nova-core/src/window.rs`:
  ```rust
  pub struct WindowConfig {
      pub title: String,
      pub inner_size: Option<LogicalSize>,
      pub resizable: bool,
      pub maximized: bool,
      pub visible: bool,
      pub decorations: bool,
      pub transparent: bool,
  }
  ```
  with a `Default` and builder-style methods (`with_title`, `with_inner_size`, etc.).
- [x] Implement `From<WindowConfig> for winit::window::WindowAttributes`.
- [x] Add `ApplicationBuilder::window_config(WindowConfig)` replacing `alter_window_attributes`.
  - Keep `alter_window_attributes` as `pub(crate)` or remove it if no internal callers remain.
- [x] Keep `LogicalSize` and `PhysicalSize` re-exports (already in `window.rs`).
- [x] Update all call sites (nova-test, nova-editor, stress test).

**Files affected**:
- `nova-core/src/window.rs` — new `WindowConfig` struct + `From` impl
- `nova-core/src/app/builder.rs` — replace `alter_window_attributes` with `window_config`
- `nova-core/src/app.rs` — `Application` stores `WindowConfig` instead of `WindowAttributes`
- `nova-core/src/app/handler.rs` — convert `WindowConfig` → `WindowAttributes` at init
- `nova-test/src/main.rs` — update window setup
- `nova-editor/src/main.rs` — update window setup
- `nova-test/src/stress.rs` — update window setup

---

### R4. Remove inline wgpu enum mapping from `ApplicationContext`

**Problem**: `ApplicationContext::surface_format()` manually maps `wgpu::TextureFormat` → `TextureFormat` with a match block. `ApplicationContext::register_texture()` manually maps `FilterMode` → `wgpu::FilterMode`.

**Tasks**:

- [x] Add `From<wgpu::TextureFormat> for TextureFormat` in `texture.rs` — covers all variants in the engine enum, fallback to `Bgra8UnormSrgb`.
- [x] Update `surface_format()` to use `.into()`.
- [x] Update `register_texture()` to use `FilterMode::into()` (the `From<FilterMode> for wgpu::FilterMode` impl already exists in `sampler.rs`).
- [x] Update `EguiState::register_texture` to take engine-native `FilterMode` instead of `wgpu::FilterMode`.

**Files affected**:
- `nova-core/src/graphics/texture.rs` — add `From<wgpu::TextureFormat>`
- `nova-core/src/app.rs` — simplify `surface_format()` and `register_texture()`
- `nova-core/src/egui.rs` — take engine-native `FilterMode`

---

### R5. Remove `wgpu` re-export from `nova-core` lib

**Problem**: `nova-core` does not explicitly re-export `wgpu`, but the `pub(crate)` fields on `RenderContext` and `GraphicsContext` are `pub(crate)`, so they don't leak. However, the `TextureRenderTarget`, `RenderTarget`, and `Frame` types expose `wgpu::TextureView` in their public API signatures (e.g. `RenderTarget::new(RefMut, &wgpu::TextureView)`).

**Tasks**:

- [x] Audit all public API signatures in `nova-core/src/graphics/` for `wgpu::` types.
- [x] For `RenderTarget::new` — it takes `&wgpu::TextureView`. This is called internally by `Frame::render_target` and `TextureRenderTarget::as_render_target`. Make it `pub(crate)` since external code should use those wrapper methods.
- [x] For `TextureRenderTarget::view()` — returns `&wgpu::TextureView`. This is used by `EguiState::register_texture` (pub(crate)). Make it `pub(crate)`.
- [x] For `TextureRenderTarget::texture()` — returns `&wgpu::Texture`. Currently unused externally. Make it `pub(crate)`.
- [x] Verify no app-facing API exposes wgpu types after these changes.

**Files affected**:
- `nova-core/src/graphics/render_target.rs` — tighten visibility
- `nova-core/src/graphics/frame.rs` — verify `view()` is `pub` but returns `&wgpu::TextureView` (used internally only, make `pub(crate)`)

---

## Execution Order

1. **R1** — GraphicsConfiguration enums (small, isolated, no API break for internal code)
2. **R2** — ControlFlow enum (small, one new type)
3. **R4** — Remove inline wgpu mapping (cleanup, depends on R1 being done first for consistency)
4. **R3** — WindowConfig (larger, touches builder + all apps)
5. **R5** — Tighten wgpu visibility (audit pass, no app changes)

---

## Detailed Roadmap Steps (ordered by execution priority)

### Phase 0 — Refactors (above)

Complete R1–R5 before any new features. These are prerequisites for clean API boundaries.

---

### Phase 1 — Input System (nova-core)

> **Priority**: Highest after refactors. The editor cannot be interactive without input.

#### 1.1 Input state tracking

- [x] Create `nova-core/src/input.rs` module.
- [x] Define `InputState` struct:
  - `keyboard: HashMap<KeyCode, ElementState>` — current key states.
  - `mouse_position: Vec2` — logical pixel position.
  - `mouse_buttons: [ElementState; 3]` — left, right, middle.
  - `mouse_delta: Vec2` — movement since last frame.
  - `scroll_delta: f32` — vertical scroll amount.
  - `text_input: String` — accumulated text since last frame.
- [x] Define engine-native `KeyCode` enum (mirror of `winit::event::KeyCode` — only the keys we need, extensible).
- [x] Define engine-native `ElementState { Pressed, Released }`.
- [x] Define engine-native `MouseButton { Left, Right, Middle, Other(u16) }`.
- [x] Implement `From<winit::event::KeyCode>` and `From<winit::event::ElementState>` etc.
- [x] `InputState::new()` — empty state.
- [x] `InputState::process_event(&mut self, event: &WindowEvent)` — update state from winit events.
- [x] `InputState::begin_frame(&mut self)` — clear per-frame deltas (mouse_delta, scroll_delta, text_input).
- [x] `InputState::is_key_down(&self, key: KeyCode) -> bool`.
- [x] `InputState::is_mouse_button_down(&self, button: MouseButton) -> bool`.
- [x] `InputState::mouse_position(&self) -> Vec2`.

#### 1.2 Input integration in event loop

- [x] Add `input_state: InputState` field to `ApplicationContext`.
- [x] In `handler.rs::process_events`: call `input_state.process_event(&event)` for all events (before egui, after egui for unconsumed events).
- [x] In `handler.rs::on_render` (or `on_update`): call `input_state.begin_frame()` at the start of each frame.
- [x] Expose `InputState` via `ApplicationContext` (public field or accessor).

#### 1.3 Input access in ApplicationProxy

- [x] `ApplicationContext` exposes `&InputState` (read-only in `on_render`, read-only in `on_update`).
- [x] `ApplicationContext` exposes `&mut InputState` in `on_update` for apps that want to consume events.

#### 1.4 Egui + input coexistence

- [x] After egui processes an event, check `egui::Context::wants_pointer_input()` / `wants_keyboard_input()`.
- [x] If egui wants the event, don't forward it to `InputState` (the game/editor viewport shouldn't receive UI clicks).
- [x] If egui doesn't want it, forward to `InputState` normally.

---

### Phase 2 — GPU-side Transform Construction (nova-2d)

> **Priority**: High. Eliminates the #1 documented CPU bottleneck.

#### 2.1 Refactor `RectInstance`

- [ ] Replace `transform: Mat3` (36 bytes) with `position: [f32; 2]`, `angle: f32`, `scale: [f32; 2]` (20 bytes).
- [ ] Update `RectangleShape::build_instance` to produce the new struct.
- [ ] Update `SpriteAtlas` — no change needed (uses `ShapeInstance<RectangleShape>` which calls `build_instance`).

#### 2.2 Update `SpriteMaterial` template

- [ ] Update `instance_layout` in `SpriteMaterial::template()`:
  - Was: `Float32x3, Float32x3, Float32x3, Float32x4, Float32x4` (mat3 + color + uv)
  - New: `Float32x2, Float32, Float32x2, Float32x4, Float32x4` (position + angle + scale + color + uv)

#### 2.3 Update `sprite_shader.wgsl`

- [ ] Vertex shader: reconstruct `mat3` from `(position, angle, scale)`:
  ```wgsl
  let c = cos(angle);
  let s = sin(angle);
  let rot = mat3x3<f32>(
      vec3(c * scale.x, s * scale.x, 0.0),
      vec3(-s * scale.y, c * scale.y, 0.0),
      vec3(position.x, position.y, 1.0),
  );
  let transformed = rot * vec3(position, 1.0);
  ```
- [ ] Update `rectangle_shader.wgsl` (ColorMaterial) with the same change.

#### 2.4 Verify

- [ ] Run nova-test — sprites should render identically.
- [ ] Run stress test — measure draw_ms improvement (expect ~40-50ns/quad reduction).
- [ ] Run nova-editor — animated shapes should render correctly.

---

### Phase 3 — Window DPI & Resize (nova-core + nova-2d)

> **Priority**: Medium. Needed for correct rendering on high-DPI displays.

#### 3.1 DPI scale propagation

- [ ] Add `scale_factor: f32` to `ApplicationContext` (updated on `ScaleFactorChanged` event).
- [ ] Expose via `ApplicationContext::scale_factor()`.
- [ ] `Camera2D` takes logical size (already does via `WindowApi::size()`). Verify this is correct.

#### 3.2 Resize event handling

- [ ] `WindowEvent::Resized` already calls `resize_surface`. Verify it also updates any cached sizes.
- [ ] Expose resize event to `ApplicationProxy` via a callback or a flag on `ApplicationContext`.
- [ ] `nova-editor`: recreate `TextureRenderTarget` on resize (or keep fixed 800×600 and scale in egui).

#### 3.3 Window lifecycle events

- [ ] Add optional `on_event(&mut self, ctx, event: &EngineEvent)` to `ApplicationProxy`.
- [ ] Define `EngineEvent` enum: `Resized(LogicalSize)`, `Focused`, `Unfocused`, `CloseRequested`, `ScaleFactorChanged(f32)`.
- [ ] Map winit events to `EngineEvent` in handler.

---

### Phase 4 — Frustum Culling (nova-2d)

> **Priority**: Medium. Skips off-screen instances.

#### 4.1 Camera visible bounds

- [ ] Add `Camera2D::visible_bounds() -> (Vec2, Vec2)` — returns the min/max world-space bounds visible through the camera (accounting for zoom, rotation, viewport).
- [ ] For rotation: compute the AABB of the rotated viewport rectangle.

#### 4.2 Per-shape culling test

- [ ] Add `Shape2D::cull_bounds(position, scale) -> RectF32` — returns the world-space AABB of a shape instance.
- [ ] For `RectangleShape`: AABB = position ± scale/2 (axis-aligned, ignoring rotation for simplicity — conservative).
- [ ] For `CircleShape`: AABB = position ± radius.

#### 4.3 Cull in batcher

- [ ] `Batcher2D::add` takes an optional camera bounds parameter (or the `Render2D::draw` method checks before calling `batcher.add`).
- [ ] Skip `batcher.add` if the shape's AABB doesn't intersect the camera's visible bounds.

---

### Phase 5 — More Shapes (nova-2d)

> **Priority**: Medium. Extends the shape system.

#### 5.1 `LineShape`

- [ ] `LineInstance { start: [f32;2], end: [f32;2], thickness: f32, color: [f32;4] }`.
- [ ] `LineMaterial` — no bind group data (color per-instance).
- [ ] `line_shader.wgsl` — vertex shader maps quad to a line quad (oriented rectangle from start to end with thickness).
- [ ] Register in `Nova2DPlugin`.

#### 5.2 `RingShape`

- [ ] `RingInstance { position: [f32;2], outer_radius: f32, inner_radius: f32, color: [f32;4] }`.
- [ ] `RingMaterial` — no bind group data.
- [ ] `ring_shader.wgsl` — fragment discards if `dist < inner_radius || dist > outer_radius`.

#### 5.3 `RoundedRectShape`

- [ ] `RoundedRectInstance { transform: Mat3, color: [f32;4], corner_radius: f32 }`.
- [ ] `RoundedRectMaterial` — no bind group data.
- [ ] `rounded_rect_shader.wgsl` — SDF for rounded rectangle in fragment shader.

---

### Phase 6 — Text Rendering (nova-2d)

> **Priority**: Medium. Essential for UI and debug overlays.

#### 6.1 Glyph atlas

- [ ] Use `ab_glyph` or `rusttype` for CPU-side glyph rasterization.
- [ ] `GlyphAtlas` struct — caches rendered glyphs as a texture atlas (CPU `Texture`).
- [ ] Atlas grows dynamically as new glyphs are requested.

#### 6.2 Text layout

- [ ] `TextLayout` struct — computes positions for each glyph given a string, font size, alignment, wrapping.
- [ ] Returns a list of `(glyph_rect, uv_rect)` pairs.

#### 6.3 Text rendering

- [ ] `TextRenderer` — takes a `TextLayout` + `SpriteMaterial` (the glyph atlas texture), emits `ShapeInstance<RectangleShape>` per glyph.
- [ ] Integrates with `Render2D::draw` — text is just a batch of textured rectangles.

---

### Phase 7 — Scene System (nova-2d or nova-scene)

> **Priority**: Medium. Organizational layer for real applications.

#### 7.1 Transform hierarchy

- [ ] `Transform2D` struct — local position, rotation, scale + cached world matrix.
- [ ] Parent-child relationships via indices (not references) — `parent: Option<usize>`.
- [ ] `update_world_transforms(&mut transforms)` — propagate parent → child, dirty flag optimization.

#### 7.2 Entity structure

- [ ] `Entity2D` struct — `transform: Transform2D`, `shape: ShapeKind`, `material: GenericHandle`, `visible: bool`, `z_index: u32`.
- [ ] `Scene2D` struct — `Vec<Entity2D>`, `Vec<Transform2D>` (separate arrays for cache efficiency).

#### 7.3 Scene → Render2D bridge

- [ ] `Scene2D::render(&self, renderer: &mut Render2D, camera: &Camera2D)`.
- [ ] Iterates visible entities, builds `ShapeInstance` from entity data + world transform, calls `renderer.draw`.

#### 7.4 Editor integration

- [ ] Editor owns a `Scene2D`, renders it in the viewport.
- [ ] Inspector panel edits selected entity's transform/shape/material.
- [ ] Entity list panel shows all entities, click to select.