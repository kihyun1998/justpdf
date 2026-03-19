use tiny_skia::{
    BlendMode, Color, FillRule, Mask, Paint, Path, Pixmap, PixmapPaint, Stroke, Transform,
};

/// A single recorded rendering command.
#[derive(Debug, Clone)]
pub enum DisplayCommand {
    /// Fill a path.
    FillPath {
        path: Path,
        fill_rule: FillRule,
        transform: Transform,
        color: Color,
        alpha: f32,
        blend_mode: BlendMode,
    },
    /// Stroke a path.
    StrokePath {
        path: Path,
        stroke: Stroke,
        transform: Transform,
        color: Color,
        alpha: f32,
        blend_mode: BlendMode,
    },
    /// Draw an image.
    DrawImage {
        pixmap: Pixmap,
        transform: Transform,
        alpha: f32,
        blend_mode: BlendMode,
    },
    /// Push a clip path.
    PushClip {
        path: Path,
        fill_rule: FillRule,
        transform: Transform,
    },
    /// Pop the last clip path.
    PopClip,
    /// Save graphics state.
    Save,
    /// Restore graphics state.
    Restore,
    /// Begin a transparency group.
    BeginGroup {
        opacity: f32,
        blend_mode: BlendMode,
        isolated: bool,
    },
    /// End a transparency group.
    EndGroup,
}

/// A recorded list of rendering commands that can be replayed onto a pixmap.
///
/// The display list captures rendering operations so they can be executed later,
/// potentially multiple times or at different scales. This is useful for caching
/// page rendering results, implementing tiled rendering, and print preview.
#[derive(Debug, Clone)]
pub struct DisplayList {
    commands: Vec<DisplayCommand>,
    width: u32,
    height: u32,
}

impl DisplayList {
    /// Create a new empty display list with the given target dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            commands: Vec::new(),
            width,
            height,
        }
    }

    /// Record a command.
    pub fn push(&mut self, cmd: DisplayCommand) {
        self.commands.push(cmd);
    }

    /// Number of recorded commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns true if no commands have been recorded.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get all commands.
    pub fn commands(&self) -> &[DisplayCommand] {
        &self.commands
    }

    /// Replay all commands onto a pixmap using the identity transform.
    pub fn replay(&self, pixmap: &mut Pixmap) {
        self.replay_with_transform(pixmap, Transform::identity());
    }

    /// Replay all commands onto a pixmap with an extra transform applied
    /// to every drawing operation. This is useful for rendering at different
    /// scales or offsets without re-recording the display list.
    pub fn replay_with_transform(&self, pixmap: &mut Pixmap, extra_transform: Transform) {
        let mut clip_stack: Vec<Mask> = Vec::new();
        let mut current_mask: Option<Mask> = None;
        let mut group_stack: Vec<GroupState> = Vec::new();

        for cmd in &self.commands {
            match cmd {
                DisplayCommand::FillPath {
                    path,
                    fill_rule,
                    transform,
                    color,
                    alpha,
                    blend_mode,
                } => {
                    let target = group_target(&mut group_stack, pixmap);
                    let combined = extra_transform.post_concat(*transform);
                    let mut paint = Paint::default();
                    let c = apply_alpha(*color, *alpha);
                    paint.set_color(c);
                    paint.anti_alias = true;
                    paint.blend_mode = *blend_mode;
                    let mask_ref = current_mask.as_ref();
                    target.fill_path(path, &paint, *fill_rule, combined, mask_ref);
                }

                DisplayCommand::StrokePath {
                    path,
                    stroke,
                    transform,
                    color,
                    alpha,
                    blend_mode,
                } => {
                    let target = group_target(&mut group_stack, pixmap);
                    let combined = extra_transform.post_concat(*transform);
                    let mut paint = Paint::default();
                    let c = apply_alpha(*color, *alpha);
                    paint.set_color(c);
                    paint.anti_alias = true;
                    paint.blend_mode = *blend_mode;
                    let mask_ref = current_mask.as_ref();
                    target.stroke_path(path, &paint, stroke, combined, mask_ref);
                }

                DisplayCommand::DrawImage {
                    pixmap: img,
                    transform,
                    alpha,
                    blend_mode,
                } => {
                    let target = group_target(&mut group_stack, pixmap);
                    let combined = extra_transform.post_concat(*transform);
                    let mut ppaint = PixmapPaint::default();
                    ppaint.opacity = *alpha;
                    ppaint.blend_mode = *blend_mode;
                    ppaint.quality = tiny_skia::FilterQuality::Bilinear;
                    let mask_ref = current_mask.as_ref();
                    target.draw_pixmap(0, 0, img.as_ref(), &ppaint, combined, mask_ref);
                }

                DisplayCommand::PushClip {
                    path,
                    fill_rule,
                    transform,
                } => {
                    // Save the current mask before pushing.
                    if let Some(m) = current_mask.take() {
                        clip_stack.push(m);
                    }
                    let target = group_target(&mut group_stack, pixmap);
                    let w = target.width();
                    let h = target.height();
                    let combined = extra_transform.post_concat(*transform);
                    if let Some(mut mask) = Mask::new(w, h) {
                        mask.fill_path(path, *fill_rule, true, combined);
                        // Intersect with the previous mask if there was one.
                        if let Some(prev) = clip_stack.last() {
                            intersect_masks(&mut mask, prev);
                        }
                        current_mask = Some(mask);
                    }
                }

                DisplayCommand::PopClip => {
                    current_mask = clip_stack.pop();
                }

                DisplayCommand::Save => {
                    // Save is a no-op for replay since we track clips explicitly.
                }

                DisplayCommand::Restore => {
                    // Restore is a no-op for replay since we track clips explicitly.
                }

                DisplayCommand::BeginGroup {
                    opacity,
                    blend_mode,
                    isolated: _,
                } => {
                    let target = group_target(&mut group_stack, pixmap);
                    let w = target.width();
                    let h = target.height();
                    if let Some(group_pixmap) = Pixmap::new(w, h) {
                        group_stack.push(GroupState {
                            pixmap: group_pixmap,
                            opacity: *opacity,
                            blend_mode: *blend_mode,
                        });
                    }
                }

                DisplayCommand::EndGroup => {
                    if let Some(group) = group_stack.pop() {
                        let target = group_target(&mut group_stack, pixmap);
                        let mut ppaint = PixmapPaint::default();
                        ppaint.opacity = group.opacity;
                        ppaint.blend_mode = group.blend_mode;
                        let mask_ref = current_mask.as_ref();
                        target.draw_pixmap(
                            0,
                            0,
                            group.pixmap.as_ref(),
                            &ppaint,
                            Transform::identity(),
                            mask_ref,
                        );
                    }
                }
            }
        }
    }

    /// Compute the axis-aligned bounding box that encloses all paths in the
    /// display list. Returns `None` if no paths have been recorded.
    pub fn bounds(&self) -> Option<tiny_skia::Rect> {
        let mut result: Option<tiny_skia::Rect> = None;

        for cmd in &self.commands {
            let path_bounds = match cmd {
                DisplayCommand::FillPath { path, .. }
                | DisplayCommand::StrokePath { path, .. }
                | DisplayCommand::PushClip { path, .. } => Some(path.bounds()),
                _ => None,
            };

            if let Some(b) = path_bounds {
                result = Some(match result {
                    Some(r) => union_rect(r, b),
                    None => b,
                });
            }
        }

        result
    }

    /// Remove redundant command sequences that have no visual effect.
    ///
    /// Currently optimizes:
    /// - Consecutive `Save` / `Restore` pairs with nothing between them.
    /// - Consecutive `BeginGroup` / `EndGroup` pairs with nothing between them.
    pub fn optimize(&mut self) {
        loop {
            let before = self.commands.len();
            let mut optimized = Vec::with_capacity(self.commands.len());
            let mut i = 0;
            while i < self.commands.len() {
                if i + 1 < self.commands.len() {
                    let is_noop_pair = matches!(
                        (&self.commands[i], &self.commands[i + 1]),
                        (DisplayCommand::Save, DisplayCommand::Restore)
                            | (DisplayCommand::BeginGroup { .. }, DisplayCommand::EndGroup)
                            | (DisplayCommand::PushClip { .. }, DisplayCommand::PopClip)
                    );
                    if is_noop_pair {
                        i += 2;
                        continue;
                    }
                }
                optimized.push(self.commands[i].clone());
                i += 1;
            }
            self.commands = optimized;
            // Repeat until stable (nested removals may expose new pairs).
            if self.commands.len() == before {
                break;
            }
        }
    }

    /// Width of the target surface in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the target surface in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Tracks state for a transparency group during replay.
struct GroupState {
    pixmap: Pixmap,
    opacity: f32,
    blend_mode: BlendMode,
}

/// Returns a mutable reference to the top-most group pixmap, or to the root
/// pixmap when there is no active group.
fn group_target<'a>(stack: &'a mut Vec<GroupState>, root: &'a mut Pixmap) -> &'a mut Pixmap {
    if let Some(top) = stack.last_mut() {
        &mut top.pixmap
    } else {
        root
    }
}

/// Apply an alpha multiplier to a color's existing alpha channel.
fn apply_alpha(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.red(), color.green(), color.blue(), color.alpha() * alpha)
        .unwrap_or(color)
}

/// Compute the union of two axis-aligned rectangles.
fn union_rect(a: tiny_skia::Rect, b: tiny_skia::Rect) -> tiny_skia::Rect {
    let l = a.left().min(b.left());
    let t = a.top().min(b.top());
    let r = a.right().max(b.right());
    let bot = a.bottom().max(b.bottom());
    tiny_skia::Rect::from_ltrb(l, t, r, bot).unwrap_or(a)
}

/// Intersect mask `dst` with `src` by AND-ing their alpha values.
fn intersect_masks(dst: &mut Mask, src: &Mask) {
    let dst_data = dst.data_mut();
    let src_data = src.data();
    let len = dst_data.len().min(src_data.len());
    for i in 0..len {
        dst_data[i] = ((dst_data[i] as u16 * src_data[i] as u16) / 255) as u8;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{PathBuilder, Stroke};

    /// Helper: create a simple rectangular path.
    fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
        let mut pb = PathBuilder::new();
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
        pb.finish().unwrap()
    }

    #[test]
    fn test_empty_display_list() {
        let dl = DisplayList::new(100, 100);
        assert!(dl.is_empty());
        assert_eq!(dl.len(), 0);
        assert_eq!(dl.width(), 100);
        assert_eq!(dl.height(), 100);
        assert!(dl.bounds().is_none());
    }

    #[test]
    fn test_record_and_replay_fill() {
        let mut dl = DisplayList::new(100, 100);
        dl.push(DisplayCommand::FillPath {
            path: rect_path(10.0, 10.0, 50.0, 50.0),
            fill_rule: FillRule::Winding,
            transform: Transform::identity(),
            color: Color::from_rgba8(255, 0, 0, 255),
            alpha: 1.0,
            blend_mode: BlendMode::SourceOver,
        });
        assert_eq!(dl.len(), 1);

        let mut pixmap = Pixmap::new(100, 100).unwrap();
        pixmap.fill(Color::from_rgba8(255, 255, 255, 255));
        dl.replay(&mut pixmap);

        // The centre of the filled rectangle should be red.
        let pixel = pixmap.pixel(35, 35).unwrap();
        assert_eq!(pixel.red(), 255);
        assert_eq!(pixel.green(), 0);
        assert_eq!(pixel.blue(), 0);
    }

    #[test]
    fn test_record_and_replay_stroke() {
        let mut dl = DisplayList::new(100, 100);
        let mut stroke = Stroke::default();
        stroke.width = 4.0;

        dl.push(DisplayCommand::StrokePath {
            path: rect_path(10.0, 10.0, 80.0, 80.0),
            stroke,
            transform: Transform::identity(),
            color: Color::from_rgba8(0, 0, 255, 255),
            alpha: 1.0,
            blend_mode: BlendMode::SourceOver,
        });
        assert_eq!(dl.len(), 1);

        let mut pixmap = Pixmap::new(100, 100).unwrap();
        pixmap.fill(Color::from_rgba8(255, 255, 255, 255));
        dl.replay(&mut pixmap);

        // A pixel on the top edge of the stroke (y=10) should be blue.
        let pixel = pixmap.pixel(50, 10).unwrap();
        assert_eq!(pixel.blue(), 255);
    }

    #[test]
    fn test_display_list_length() {
        let mut dl = DisplayList::new(10, 10);
        assert_eq!(dl.len(), 0);

        dl.push(DisplayCommand::Save);
        assert_eq!(dl.len(), 1);

        dl.push(DisplayCommand::FillPath {
            path: rect_path(0.0, 0.0, 5.0, 5.0),
            fill_rule: FillRule::Winding,
            transform: Transform::identity(),
            color: Color::BLACK,
            alpha: 1.0,
            blend_mode: BlendMode::SourceOver,
        });
        assert_eq!(dl.len(), 2);

        dl.push(DisplayCommand::Restore);
        assert_eq!(dl.len(), 3);
    }

    #[test]
    fn test_optimize_removes_redundant_save_restore() {
        let mut dl = DisplayList::new(10, 10);
        dl.push(DisplayCommand::Save);
        dl.push(DisplayCommand::Restore);
        dl.push(DisplayCommand::FillPath {
            path: rect_path(0.0, 0.0, 5.0, 5.0),
            fill_rule: FillRule::Winding,
            transform: Transform::identity(),
            color: Color::BLACK,
            alpha: 1.0,
            blend_mode: BlendMode::SourceOver,
        });
        dl.push(DisplayCommand::Save);
        dl.push(DisplayCommand::Restore);

        assert_eq!(dl.len(), 5);
        dl.optimize();
        // Both empty Save/Restore pairs should be removed.
        assert_eq!(dl.len(), 1);
        assert!(matches!(dl.commands()[0], DisplayCommand::FillPath { .. }));
    }

    #[test]
    fn test_optimize_nested_noop() {
        // Save, Save, Restore, Restore should reduce to nothing.
        let mut dl = DisplayList::new(10, 10);
        dl.push(DisplayCommand::Save);
        dl.push(DisplayCommand::Save);
        dl.push(DisplayCommand::Restore);
        dl.push(DisplayCommand::Restore);

        dl.optimize();
        assert!(dl.is_empty());
    }

    #[test]
    fn test_replay_with_transform_scales() {
        let mut dl = DisplayList::new(200, 200);
        dl.push(DisplayCommand::FillPath {
            path: rect_path(0.0, 0.0, 50.0, 50.0),
            fill_rule: FillRule::Winding,
            transform: Transform::identity(),
            color: Color::from_rgba8(0, 255, 0, 255),
            alpha: 1.0,
            blend_mode: BlendMode::SourceOver,
        });

        let mut pixmap = Pixmap::new(200, 200).unwrap();
        pixmap.fill(Color::from_rgba8(0, 0, 0, 255));

        // Replay at 2x scale: the 50x50 rect becomes 100x100.
        let scale = Transform::from_scale(2.0, 2.0);
        dl.replay_with_transform(&mut pixmap, scale);

        // Pixel at (75, 75) should be green (inside the scaled 100x100 rect).
        let inside = pixmap.pixel(75, 75).unwrap();
        assert_eq!(inside.green(), 255);

        // Pixel at (150, 150) should still be black (outside the scaled rect).
        let outside = pixmap.pixel(150, 150).unwrap();
        assert_eq!(outside.red(), 0);
        assert_eq!(outside.green(), 0);
        assert_eq!(outside.blue(), 0);
    }

    #[test]
    fn test_bounds() {
        let mut dl = DisplayList::new(100, 100);
        dl.push(DisplayCommand::FillPath {
            path: rect_path(10.0, 20.0, 30.0, 40.0),
            fill_rule: FillRule::Winding,
            transform: Transform::identity(),
            color: Color::BLACK,
            alpha: 1.0,
            blend_mode: BlendMode::SourceOver,
        });
        dl.push(DisplayCommand::StrokePath {
            path: rect_path(50.0, 60.0, 10.0, 5.0),
            stroke: Stroke::default(),
            transform: Transform::identity(),
            color: Color::BLACK,
            alpha: 1.0,
            blend_mode: BlendMode::SourceOver,
        });

        let b = dl.bounds().unwrap();
        assert!((b.left() - 10.0).abs() < 0.01);
        assert!((b.top() - 20.0).abs() < 0.01);
        assert!((b.right() - 60.0).abs() < 0.01);
        assert!((b.bottom() - 65.0).abs() < 0.01);
    }
}
