//! Which tool is currently driving paint events.
//!
//! [`ActiveTool`] is a Bevy [`Resource`] that consumer apps write from
//! their keybind / palette handlers and that brush / region systems
//! read with `run_if` to decide whether to consume the current pointer
//! stream.
//!
//! v0.2 ships two variants — `Brush` and `Rect`. Marked
//! `#[non_exhaustive]` so adding a `Lasso` / `Pen` / `Slice` later is
//! not a breaking change. If three tools' lifecycles ever diverge
//! enough to justify dynamic dispatch, this can become a
//! `Box<dyn Tool>` resource without affecting `BrushOp` / `Region`.

use bevy_ecs::prelude::*;
use bevy_ecs::reflect::ReflectResource;
use bevy_reflect::Reflect;

/// Which tool is currently driving paint events.
#[derive(Resource, Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Resource)]
#[non_exhaustive]
pub enum ActiveTool {
    /// Falloff-disk brush. Reads [`crate::ActiveBrush`].
    #[default]
    Brush,
    /// Marquee-style rectangle. The drag state machine + commit math
    /// live in the consumer — `bevy_cad` only owns the
    /// [`crate::Region`] data and the [`crate::apply_brush_to_region`]
    /// helper.
    Rect,
}
