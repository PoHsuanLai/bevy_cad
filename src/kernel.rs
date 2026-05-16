//! Pure radius / falloff math used by brush handlers.
//!
//! No Bevy deps beyond `Vec2`/`UVec2` and `Reflect` on [`FalloffCurve`].
//! Tested as pure functions; consumers compose them with whatever
//! per-cell mutation rule fits the domain.

use bevy_math::{UVec2, Vec2};
use bevy_reflect::Reflect;

/// Which falloff curve a brush uses when weighting cells by distance.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FalloffCurve {
    /// Hermite smoothstep — soft Gaussian-ish dab. Sensible default for
    /// paint brushes.
    #[default]
    Smoothstep,
    /// Constant slope from 1.0 at center to 0.0 at radius. Reads as a
    /// harder ring; useful for marquee-style selections.
    Linear,
}

impl FalloffCurve {
    /// Evaluate the curve at `distance` for the given `radius`. Returns
    /// values in `[0, 1]`. Distances `>= radius` return 0.
    pub fn eval(self, distance: f32, radius: f32) -> f32 {
        match self {
            Self::Smoothstep => smoothstep_falloff(distance, radius),
            Self::Linear => linear_falloff(distance, radius),
        }
    }
}

/// Iterate `(x, y, falloff)` for every grid cell within `radius` of
/// `center`, where `falloff` ∈ `[0, 1]` decreases smoothly from `1.0`
/// at the center to `0.0` at the radius.
///
/// `center` is in cell coordinates and may be fractional — useful when
/// projecting a continuous pointer hit onto the discrete grid. `dims`
/// clamps the iteration to `[0, width) × [0, height)`.
pub fn cells_in_radius(
    center: Vec2,
    radius: f32,
    dims: (u32, u32),
) -> impl Iterator<Item = (u32, u32, f32)> {
    let (w, h) = dims;
    let r = radius.max(0.0);
    let r2 = r * r;
    let x0 = ((center.x - r).floor()).max(0.0) as i32;
    let y0 = ((center.y - r).floor()).max(0.0) as i32;
    let x1 = ((center.x + r).ceil()).min(w as f32) as i32;
    let y1 = ((center.y + r).ceil()).min(h as f32) as i32;

    (y0..y1).flat_map(move |y| {
        (x0..x1).filter_map(move |x| {
            let dx = x as f32 + 0.5 - center.x;
            let dy = y as f32 + 0.5 - center.y;
            let d2 = dx * dx + dy * dy;
            if d2 > r2 {
                return None;
            }
            let falloff = smoothstep_falloff(d2.sqrt(), r);
            Some((x as u32, y as u32, falloff))
        })
    })
}

/// Smoothstep falloff: `1.0` at distance 0, `0.0` at `radius`, smooth
/// derivatives at both ends.
pub fn smoothstep_falloff(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let t = (distance / radius).clamp(0.0, 1.0);
    let s = 1.0 - t;
    s * s * (3.0 - 2.0 * s)
}

/// Linear falloff: `1.0` at distance 0, `0.0` at `radius`, constant slope.
pub fn linear_falloff(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    (1.0 - (distance / radius).clamp(0.0, 1.0)).max(0.0)
}

/// Convenience: integer-cell version of [`cells_in_radius`] for callers
/// that already have a discrete center cell.
pub fn cells_in_radius_uvec(
    center: UVec2,
    radius: f32,
    dims: (u32, u32),
) -> impl Iterator<Item = (u32, u32, f32)> {
    cells_in_radius(
        Vec2::new(center.x as f32 + 0.5, center.y as f32 + 0.5),
        radius,
        dims,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_zero_yields_no_cells() {
        assert_eq!(
            cells_in_radius(Vec2::new(5.0, 5.0), 0.0, (10, 10)).count(),
            0
        );
    }

    #[test]
    fn radius_one_at_cell_center_includes_only_that_cell() {
        let cells: Vec<_> = cells_in_radius(Vec2::new(5.5, 5.5), 0.4, (10, 10)).collect();
        assert_eq!(cells, vec![(5, 5, 1.0)]);
    }

    #[test]
    fn radius_three_covers_circle() {
        let cells: Vec<_> = cells_in_radius(Vec2::new(5.5, 5.5), 3.0, (10, 10)).collect();
        assert!(cells.len() > 9);
        for (x, y, f) in &cells {
            assert!(*x < 10 && *y < 10);
            assert!(*f >= 0.0 && *f <= 1.0);
        }
    }

    #[test]
    fn falloff_decreases_with_distance() {
        let center = Vec2::new(5.5, 5.5);
        let cells: Vec<_> = cells_in_radius(center, 4.0, (12, 12)).collect();
        let center_cell = cells.iter().find(|(x, y, _)| *x == 5 && *y == 5).unwrap();
        let far_cell = cells
            .iter()
            .max_by(|a, b| {
                let da =
                    ((a.0 as f32 + 0.5 - 5.5).powi(2) + (a.1 as f32 + 0.5 - 5.5).powi(2)).sqrt();
                let db =
                    ((b.0 as f32 + 0.5 - 5.5).powi(2) + (b.1 as f32 + 0.5 - 5.5).powi(2)).sqrt();
                da.partial_cmp(&db).unwrap()
            })
            .unwrap();
        assert!(center_cell.2 > far_cell.2);
    }

    #[test]
    fn out_of_bounds_centers_clip_correctly() {
        let cells: Vec<_> = cells_in_radius(Vec2::new(11.0, 11.0), 3.0, (10, 10)).collect();
        for (x, y, _) in &cells {
            assert!(*x < 10 && *y < 10);
        }
    }

    #[test]
    fn smoothstep_endpoints() {
        assert_eq!(smoothstep_falloff(0.0, 5.0), 1.0);
        assert_eq!(smoothstep_falloff(5.0, 5.0), 0.0);
        assert_eq!(smoothstep_falloff(10.0, 5.0), 0.0);
    }

    #[test]
    fn smoothstep_is_monotone_decreasing() {
        let r = 5.0;
        let mut prev = 1.1;
        for d in 0..=20 {
            let v = smoothstep_falloff(d as f32 * 0.25, r);
            assert!(v <= prev, "d={d}: {v} > prev {prev}");
            prev = v;
        }
    }

    #[test]
    fn linear_endpoints() {
        assert_eq!(linear_falloff(0.0, 4.0), 1.0);
        assert_eq!(linear_falloff(4.0, 4.0), 0.0);
        assert_eq!(linear_falloff(8.0, 4.0), 0.0);
    }

    #[test]
    fn radius_negative_or_zero_returns_zero_falloff() {
        assert_eq!(smoothstep_falloff(1.0, 0.0), 0.0);
        assert_eq!(linear_falloff(1.0, -1.0), 0.0);
    }

    #[test]
    fn falloff_curve_eval_matches_free_functions() {
        assert_eq!(FalloffCurve::Smoothstep.eval(2.0, 5.0), smoothstep_falloff(2.0, 5.0));
        assert_eq!(FalloffCurve::Linear.eval(2.0, 5.0), linear_falloff(2.0, 5.0));
    }
}
