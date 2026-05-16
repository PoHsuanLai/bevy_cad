//! Generic brush operations + ambient brush state.
//!
//! [`BrushOp`] is the per-cell mutation rule. [`Brush`] bundles an op
//! with radius / strength / falloff. [`ActiveBrush`] holds the brush
//! tool UIs write into. [`apply_brush`] is the pure kernel that takes a
//! per-cell weight and produces the new cell value.

use bevy_ecs::prelude::*;
use bevy_ecs::reflect::ReflectResource;
use bevy_reflect::Reflect;

use crate::kernel::FalloffCurve;

/// What a brush does to one cell. Names are domain-neutral so the same
/// op can back different UI tools — spectral's "Raise" is `Add{1.0}`,
/// its "Flatten" and "Eraser" are both `PullTo{1.0}`.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum BrushOp {
    /// `prior + s * delta`. Pure additive deposit.
    Add { delta: f32 },
    /// `prior + (neighbor_avg - prior) * s`. 3×3 (or wider) blur.
    Smooth,
    /// `prior + (target - prior) * s`. Pulls cells toward `target` —
    /// reuse for "flatten to neutral" (target = 1.0), "fill" (target =
    /// max), or "zero out" (target = 0.0).
    PullTo { target: f32 },
    /// `prior * (1 + s * (factor - 1))`. Scales toward `factor`. `s=1`,
    /// `factor=0` zeroes the cell; `factor=2` doubles it.
    Multiply { factor: f32 },
    /// `prior + (value - prior) * s`. Same shape as `PullTo` but reads
    /// as "paint this color/value here" rather than "pull toward neutral."
    /// Kept as a distinct variant so UIs can choose the verb that matches.
    Set { value: f32 },
}

impl Default for BrushOp {
    fn default() -> Self {
        Self::Add { delta: 1.0 }
    }
}

/// One brush configuration: what op, how big, how strong, and which
/// falloff curve. UIs typically own a `Res<ActiveBrush>` and mutate this
/// in response to keybinds or palette changes.
#[derive(Reflect, Clone, Debug)]
pub struct Brush {
    pub op: BrushOp,
    /// Brush radius in cell units. Fractional values are fine — the
    /// kernel iterates in floor/ceil clamped bounds.
    pub radius_cells: f32,
    /// Per-stroke blend factor in `[0, 1]`. Multiplied with the falloff
    /// curve to produce the per-cell weight `apply_brush` consumes.
    pub strength: f32,
    pub falloff: FalloffCurve,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            op: BrushOp::default(),
            radius_cells: 4.0,
            strength: 0.25,
            falloff: FalloffCurve::Smoothstep,
        }
    }
}

/// The currently-active brush. Tool UIs mutate `.0`; brush handlers
/// read `.0` per pointer event.
#[derive(Resource, Default, Reflect, Debug)]
#[reflect(Resource)]
pub struct ActiveBrush(pub Brush);

/// Apply one brush op to one cell. Pure function.
///
/// `falloff_strength` is the per-cell weight the caller already produced
/// by multiplying `Brush::strength` with the falloff curve at this cell.
/// It is clamped to `[0, 1]` internally so callers don't need to.
///
/// `neighbor_avg` is only read by [`BrushOp::Smooth`]; callers without a
/// real neighborhood read can pass `prior` and the op degenerates to a
/// no-op.
pub fn apply_brush(op: BrushOp, falloff_strength: f32, prior: f32, neighbor_avg: f32) -> f32 {
    let s = falloff_strength.clamp(0.0, 1.0);
    match op {
        BrushOp::Add { delta } => prior + s * delta,
        BrushOp::Smooth => prior + (neighbor_avg - prior) * s,
        BrushOp::PullTo { target } => prior + (target - prior) * s,
        BrushOp::Multiply { factor } => prior * (1.0 + s * (factor - 1.0)),
        BrushOp::Set { value } => prior + (value - prior) * s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_deposits_strength_scaled_delta() {
        // delta=1.0, s=0.3 → +0.3
        assert!((apply_brush(BrushOp::Add { delta: 1.0 }, 0.3, 1.0, 1.0) - 1.3).abs() < 1e-5);
        // delta=2.5, s=0.5 → +1.25
        assert!((apply_brush(BrushOp::Add { delta: 2.5 }, 0.5, 0.0, 0.0) - 1.25).abs() < 1e-5);
    }

    #[test]
    fn pull_to_full_strength_lands_on_target() {
        assert!((apply_brush(BrushOp::PullTo { target: 1.0 }, 1.0, 0.0, 0.0) - 1.0).abs() < 1e-5);
        // Overshoot prior also collapses to target.
        assert!((apply_brush(BrushOp::PullTo { target: 1.0 }, 1.0, 2.0, 2.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn pull_to_half_strength_lerps() {
        // prior=0, target=1, s=0.5 → 0.5
        assert!((apply_brush(BrushOp::PullTo { target: 1.0 }, 0.5, 0.0, 0.0) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn smooth_lerps_toward_neighbor_avg() {
        // prior=0, neighbor=1, s=0.5 → 0.5
        assert!((apply_brush(BrushOp::Smooth, 0.5, 0.0, 1.0) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn smooth_with_equal_neighbor_is_noop() {
        assert!((apply_brush(BrushOp::Smooth, 1.0, 0.7, 0.7) - 0.7).abs() < 1e-5);
    }

    #[test]
    fn multiply_at_full_strength_lands_on_factor_times_prior() {
        // s=1, factor=0 → 0
        assert!(apply_brush(BrushOp::Multiply { factor: 0.0 }, 1.0, 0.5, 0.5).abs() < 1e-5);
        // s=1, factor=2 → 2*prior
        assert!((apply_brush(BrushOp::Multiply { factor: 2.0 }, 1.0, 0.5, 0.5) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn set_at_full_strength_lands_on_value() {
        assert!((apply_brush(BrushOp::Set { value: 0.7 }, 1.0, 0.0, 0.0) - 0.7).abs() < 1e-5);
    }

    #[test]
    fn falloff_strength_clamps_above_one() {
        // s=5 should be treated as s=1 — same result as full-strength PullTo.
        let r = apply_brush(BrushOp::PullTo { target: 1.0 }, 5.0, 0.0, 0.0);
        assert!((r - 1.0).abs() < 1e-5);
    }

    #[test]
    fn falloff_strength_clamps_below_zero() {
        // Negative strength clamps to zero — no change.
        let r = apply_brush(BrushOp::Add { delta: 1.0 }, -0.5, 0.42, 0.42);
        assert!((r - 0.42).abs() < 1e-5);
    }
}
