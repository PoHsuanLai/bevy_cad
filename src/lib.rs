//! CAD-style brush primitives for Bevy.
//!
//! Renderer-agnostic vocabulary for "paint a brush over a grid of cells":
//! the generic [`BrushOp`] operations, the [`ActiveBrush`] resource that
//! ambient tool state writes into, the pure [`apply_brush`] kernel, and
//! the radius/falloff math in [`kernel`].
//!
//! This crate has no opinion on *where* the brush events come from or
//! *what* the cells render as. A typical pairing is
//! [`bevy_splat`](https://crates.io/crates/bevy_splat) — its
//! `GridSplat3d` produces `GridCellHit` events that consumer code drains,
//! looks up the prior cell value, and feeds through `apply_brush`
//! together with `Res<ActiveBrush>`. Future paint targets (MIDI note
//! grids, automation lanes, …) plug in the same way.
//!
//! v0.1 scope is brush-only. Selection, snap, handle, and ruler
//! primitives belong in future releases once a second consumer pins
//! down their APIs.

use bevy_app::{App, Plugin};

pub mod brush;
pub mod kernel;
pub mod region;
pub mod tool;

pub use brush::{ActiveBrush, Brush, BrushOp, apply_brush, apply_brush_to_region};
pub use kernel::{
    FalloffCurve, cells_in_radius, cells_in_radius_uvec, linear_falloff, smoothstep_falloff,
};
pub use region::Region;
pub use tool::ActiveTool;

/// Registers [`ActiveBrush`] and [`ActiveTool`] with default settings
/// and reflects the brush/region/tool types so they show up in
/// `bevy-inspector-egui` and ECS dumps. Adds no systems — driving the
/// tools is the consumer's job.
pub struct CadPlugin;

impl Plugin for CadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveBrush>()
            .init_resource::<ActiveTool>()
            .register_type::<ActiveBrush>()
            .register_type::<Brush>()
            .register_type::<BrushOp>()
            .register_type::<FalloffCurve>()
            .register_type::<ActiveTool>()
            .register_type::<Region>();
    }
}
