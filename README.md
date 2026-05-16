# bevy_cad

CAD-style brush primitives for [Bevy](https://bevyengine.org/) 0.18.

`bevy_cad` is the renderer-agnostic vocabulary for "paint a brush over a grid of cells":

- **`BrushOp`** — `Add`, `Smooth`, `PullTo`, `Multiply`, `Set`. Domain-neutral so the same op backs different UI tools.
- **`Brush`** — op + radius + strength + falloff curve.
- **`ActiveBrush`** — `Resource` UIs mutate; brush handlers read.
- **`apply_brush(op, falloff_strength, prior, neighbor_avg) -> f32`** — the pure kernel.
- **`cells_in_radius`, falloff curves** — radius/falloff math, no Bevy deps beyond `Vec2`/`UVec2`.

The crate has **no opinion** on where brush events come from or what the cells render as. A typical pairing is [`bevy_splat`](https://crates.io/crates/bevy_splat) — its `GridSplat3d` produces `GridCellHit` events; you drain them, look up the prior cell value, and feed it through `apply_brush` with `Res<ActiveBrush>`. Future paint targets (MIDI note grids, automation lanes, …) plug in the same way.

## Quick start

```rust
use bevy::prelude::*;
use bevy_cad::{ActiveBrush, BrushOp, CadPlugin, apply_brush, cells_in_radius};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CadPlugin)
        .add_systems(Startup, |mut brush: ResMut<ActiveBrush>| {
            brush.0.op = BrushOp::PullTo { target: 1.0 };
            brush.0.radius_cells = 6.0;
            brush.0.strength = 0.4;
        })
        .run();
}
```

Inside your brush handler:

```rust
for (x, y, falloff) in cells_in_radius(center, brush.0.radius_cells, grid_dims) {
    let prior = grid.get(x, y);
    let neighbor_avg = sample_neighbor_avg(grid, x, y); // only matters for Smooth
    let next = apply_brush(brush.0.op, brush.0.strength * falloff, prior, neighbor_avg);
    grid.set(x, y, next);
}
```

## Scope

v0.1 is **brush-only**. Selection, snap, handle, and ruler primitives belong in future releases once a second consumer pins down their APIs.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).
