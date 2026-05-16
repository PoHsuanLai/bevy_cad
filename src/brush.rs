//! Generic brush operations + ambient brush state.
//!
//! [`BrushOp`] is the per-cell mutation rule. [`Brush`] bundles an op
//! with radius / strength / falloff. [`ActiveBrush`] holds the brush
//! tool UIs write into. [`apply_brush`] is the pure kernel that takes a
//! per-cell weight and produces the new cell value.

use bevy_ecs::prelude::*;
use bevy_ecs::reflect::ReflectResource;
use bevy_math::UVec2;
use bevy_reflect::Reflect;

use crate::kernel::FalloffCurve;
use crate::region::Region;

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

/// Apply one [`BrushOp`] uniformly over every cell of a [`Region`].
///
/// Unlike [`apply_brush`], there is no radius and no falloff — the
/// region itself is the area, and `strength` is the per-cell blend
/// amount (clamped to `[0, 1]` inside `apply_brush`). Marquee-style
/// edits use this: rect-select-then-gain feeds `BrushOp::Multiply`,
/// rect-select-then-mute feeds `BrushOp::Multiply { factor: 0.0 }`.
///
/// The caller exposes its grid via two closures so this helper stays
/// renderer-agnostic. `read` returns the prior cell value; `write`
/// commits the new value. Cells outside `dims` are skipped.
///
/// Smooth-style ops (`BrushOp::Smooth`) read the prior value as the
/// neighbor average — meaningful smoothing across a region needs a
/// separate two-pass pipeline that this helper deliberately does not
/// try to express.
pub fn apply_brush_to_region<R, W>(
    op: BrushOp,
    strength: f32,
    region: &Region,
    dims: (u32, u32),
    read: R,
    mut write: W,
) where
    R: Fn(UVec2) -> f32,
    W: FnMut(UVec2, f32),
{
    for cell in region.cells(dims) {
        let prior = read(cell);
        let next = apply_brush(op, strength, prior, prior);
        write(cell, next);
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

    #[test]
    fn region_multiply_zero_mutes_all_cells() {
        use std::cell::RefCell;
        let region = Region::Rect {
            min: UVec2::new(1, 1),
            max: UVec2::new(3, 3),
        };
        let grid = RefCell::new([[0.5_f32; 5]; 5]);
        apply_brush_to_region(
            BrushOp::Multiply { factor: 0.0 },
            1.0,
            &region,
            (5, 5),
            |c| grid.borrow()[c.y as usize][c.x as usize],
            |c, v| grid.borrow_mut()[c.y as usize][c.x as usize] = v,
        );
        let g = grid.borrow();
        for y in 0..5 {
            for x in 0..5 {
                let v = g[y][x];
                if (1..=3).contains(&x) && (1..=3).contains(&y) {
                    assert!(v.abs() < 1e-5, "cell ({x},{y}) should be muted, got {v}");
                } else {
                    assert!((v - 0.5).abs() < 1e-5, "cell ({x},{y}) untouched, got {v}");
                }
            }
        }
    }

    #[test]
    fn region_set_full_strength_lands_on_value() {
        use std::cell::RefCell;
        let region = Region::Rect {
            min: UVec2::new(0, 0),
            max: UVec2::new(1, 1),
        };
        let grid = RefCell::new([[0.0_f32; 2]; 2]);
        apply_brush_to_region(
            BrushOp::Set { value: 0.7 },
            1.0,
            &region,
            (2, 2),
            |c| grid.borrow()[c.y as usize][c.x as usize],
            |c, v| grid.borrow_mut()[c.y as usize][c.x as usize] = v,
        );
        for row in *grid.borrow() {
            for v in row {
                assert!((v - 0.7).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn region_skips_cells_outside_dims() {
        let region = Region::Rect {
            min: UVec2::new(8, 8),
            max: UVec2::new(12, 12),
        };
        let mut writes = 0;
        apply_brush_to_region(
            BrushOp::Add { delta: 1.0 },
            1.0,
            &region,
            (10, 10),
            |_| 0.0,
            |_, _| writes += 1,
        );
        // x in 8..=9, y in 8..=9 → 4 in-bounds cells
        assert_eq!(writes, 4);
    }
}
