//! Cell regions for marquee-style edits.
//!
//! Where [`crate::Brush`] paints a falloff disk centered on a pointer
//! hit, a [`Region`] describes a discrete set of cells defined by a
//! shape — currently a rectangle, with `Lasso` reserved for the next
//! consumer. Pair with [`crate::apply_brush_to_region`] to run any
//! [`crate::BrushOp`] uniformly across the region.

use bevy_math::UVec2;
use bevy_reflect::Reflect;

/// A discrete region of grid cells. Variants are inclusive on both
/// ends (a `Rect { min: (0,0), max: (2,2) }` covers 9 cells).
///
/// Marked `#[non_exhaustive]` so adding a `Lasso { polygon }` variant
/// later is not a breaking change.
#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Region {
    Rect { min: UVec2, max: UVec2 },
}

impl Region {
    /// Build a rectangle from two corners in any order.
    pub fn rect_from_corners(a: UVec2, b: UVec2) -> Self {
        Self::Rect {
            min: UVec2::new(a.x.min(b.x), a.y.min(b.y)),
            max: UVec2::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    /// Whether `cell` lies inside this region.
    pub fn contains(&self, cell: UVec2) -> bool {
        match self {
            Self::Rect { min, max } => {
                cell.x >= min.x && cell.x <= max.x && cell.y >= min.y && cell.y <= max.y
            }
        }
    }

    /// Tight axis-aligned bounding box, inclusive on both ends.
    pub fn aabb(&self) -> (UVec2, UVec2) {
        match self {
            Self::Rect { min, max } => (*min, *max),
        }
    }

    /// All in-bounds cells inside this region, clipped to `dims = (w, h)`.
    ///
    /// Returns a boxed iterator so the enum's variants can grow without
    /// every caller re-typing their bindings. Per-call allocation is a
    /// non-issue at typical region sizes (<10k cells); revisit if a
    /// profiler ever flags it.
    pub fn cells(&self, dims: (u32, u32)) -> Box<dyn Iterator<Item = UVec2>> {
        let (w, h) = dims;
        match *self {
            Self::Rect { min, max } => {
                let x0 = min.x;
                let y0 = min.y;
                let x1 = max.x.min(w.saturating_sub(1));
                let y1 = max.y.min(h.saturating_sub(1));
                if x0 >= w || y0 >= h || x0 > x1 || y0 > y1 {
                    return Box::new(std::iter::empty());
                }
                Box::new((y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| UVec2::new(x, y))))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_corners() {
        let r = Region::Rect {
            min: UVec2::new(2, 3),
            max: UVec2::new(5, 7),
        };
        assert!(r.contains(UVec2::new(2, 3)));
        assert!(r.contains(UVec2::new(5, 7)));
        assert!(r.contains(UVec2::new(3, 5)));
    }

    #[test]
    fn rect_excludes_outside() {
        let r = Region::Rect {
            min: UVec2::new(2, 3),
            max: UVec2::new(5, 7),
        };
        assert!(!r.contains(UVec2::new(1, 3)));
        assert!(!r.contains(UVec2::new(6, 7)));
        assert!(!r.contains(UVec2::new(2, 2)));
        assert!(!r.contains(UVec2::new(5, 8)));
    }

    #[test]
    fn rect_from_corners_normalizes_order() {
        let a = Region::rect_from_corners(UVec2::new(5, 7), UVec2::new(2, 3));
        let b = Region::rect_from_corners(UVec2::new(2, 3), UVec2::new(5, 7));
        assert_eq!(a, b);
    }

    #[test]
    fn rect_cells_iteration_count() {
        let r = Region::Rect {
            min: UVec2::new(1, 1),
            max: UVec2::new(5, 4),
        };
        // inclusive: x in 1..=5 (5), y in 1..=4 (4) → 20
        assert_eq!(r.cells((10, 10)).count(), 20);
    }

    #[test]
    fn rect_cells_clips_to_dims() {
        let r = Region::Rect {
            min: UVec2::new(8, 8),
            max: UVec2::new(20, 20),
        };
        let cells: Vec<_> = r.cells((10, 10)).collect();
        // x in 8..=9, y in 8..=9 → 4 cells
        assert_eq!(cells.len(), 4);
        for c in &cells {
            assert!(c.x < 10 && c.y < 10);
        }
    }

    #[test]
    fn rect_cells_empty_when_fully_outside() {
        let r = Region::Rect {
            min: UVec2::new(20, 20),
            max: UVec2::new(30, 30),
        };
        assert_eq!(r.cells((10, 10)).count(), 0);
    }

    #[test]
    fn aabb_matches_rect() {
        let r = Region::Rect {
            min: UVec2::new(2, 3),
            max: UVec2::new(5, 7),
        };
        assert_eq!(r.aabb(), (UVec2::new(2, 3), UVec2::new(5, 7)));
    }
}
