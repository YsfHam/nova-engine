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

- Add feature flags for selecting 2D, 3D, audio, and optional integrations.
- Add a 3D facade module when the 3D crate exists.
- Keep default plugin composition explicit and predictable as more subsystems are added.
- Provide a stable prelude only after the public API has settled.