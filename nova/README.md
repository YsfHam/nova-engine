# nova

`nova` is the workspace facade crate. It keeps application code from needing to wire the core and 2D crates separately.

## Current Scope

- `core` re-exports the complete `nova-core` API.
- `nova2d` re-exports the complete `nova-2d` API.
- `DefaultPlugins` installs `CorePlugin` followed by `Nova2DPlugin`.

The facade intentionally contains very little engine logic. Resource ownership, rendering, and asset behavior remain in the underlying crates.

## Typical Dependency

Applications can depend on `nova` and use `DefaultPlugins` as the standard starting point:

```rust
ApplicationBuilder::new(App::new())
    .with_plugins(DefaultPlugins)
    .build()
    .run()
```

## Planned Scope

### Short Term

- **Feature flags** for selecting 2D, 3D, audio, and optional integrations. Applications opt into only the subsystems they need.
- **Prelude module** — a curated re-export of the most commonly used types (`Camera2D`, `Render2D`, `Sprite`, `ShapeInstance`, `Color`, `Vec2`, etc.) so applications don't need long import lists.
- **Input facade** — re-export the input API from nova-core once implemented (nova-core Phase 1.1).
- **Scene facade** — re-export scene system types once `nova-scene` or nova-2d's scene module exists (nova-2d Phase 3).

### Medium Term

- **3D facade module** (`nova3d`) when the 3D crate exists (nova-core Phase 5).
- **Audio facade** when an audio crate is added.
- **Asset loading facade** — convenience helpers for loading textures, shaders, and materials from files (nova-core Phase 2.1).
- **Default plugin composition** — expand `DefaultPlugins` to include input handling and asset loading as they become available.

### Long Term

- **Editor integration** — optional debug/editor plugin with gizmos, inspector, and scene hierarchy view.
- **Stable API guarantee** — once the public API has settled, provide a `1.0`-style stability guarantee with deprecation cycles.
- **Platform backends** — web (WebGPU), mobile, and desktop-specific configurations surfaced through the facade.