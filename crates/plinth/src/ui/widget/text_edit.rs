use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use glamour::Point2;
use glamour::Rect;
use glamour::Size2;
use parley::PlainEditor;
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

use crate::graphics::Canvas;
use crate::graphics::ClipRect;
use crate::graphics::Color;
use crate::graphics::GradientPaint;
use crate::graphics::Paint;
use crate::graphics::Primitive;
use crate::graphics::TextAlignment;
use crate::graphics::TextLayoutContext;
use crate::shell::Input;
use crate::ui::Atom;
use crate::ui::NodeLayout;
use crate::ui::Pixels;
use crate::ui::Size;
use crate::ui::builder::UiBuilder;
use crate::ui::context::EditableTextContent;
use crate::ui::context::EditableTextVisuals;
use crate::ui::context::LayoutContent;
use crate::ui::style::BorderWidths;
use crate::ui::style::CornerRadii;
use crate::ui::style::StateFlags;
use crate::ui::style::Style;
use crate::ui::theme::StyleClass;
use crate::ui::theme::enumerate_text_style;
use crate::ui::widget::ClickBehavior;
use crate::ui::widget::Interaction;

use super::macros::forward_properties;

const DEFAULT_TEXT_EDIT_WIDTH: f32 = 120.0;

pub trait EditableTextBuffer {
    type Layout<'a>: EditableTextLayout
    where
        Self: 'a;

    fn is_empty(&self) -> bool;

    fn selected_text(&self) -> Option<&str>;

    fn apply_style(&mut self, style: &Style, state: StateFlags);

    fn enter_text(&mut self, context: &mut TextLayoutContext, text: &str);

    fn move_cursor(&mut self, context: &mut TextLayoutContext, motion: TextEditMotion);

    fn measure(
        &mut self,
        context: &mut TextLayoutContext,
        max_width: f32,
        alignment: TextAlignment,
    ) -> Option<f32>;

    fn with_layouts<'a>(
        &'a mut self,
        context: &'a mut TextLayoutContext,
        callback: impl FnMut(Self::Layout<'a>),
    );
}

pub trait EditableTextLayout {
    fn layout(&self) -> &parley::Layout<Color>;

    fn offset(&self) -> Point2<Pixels> {
        Point2::new(0.0, 0.0)
    }

    fn selection_geometry_with(&self, callback: impl FnMut(Rect<Pixels>, usize));

    fn cursor_geometry(&self, cursor_size: f32) -> Option<Rect<Pixels>>;
}

pub enum TextEditMotion {
    Backdelete,
    BackdeleteWord,
    Delete,
    DeleteWord,
    ExtendSelectionToPoint(Point2<Pixels>),
    MoveDown,
    MoveLeft,
    MoveWordLeft,
    MoveRight,
    MoveWordRight,
    MoveToLineEnd,
    MoveToLineStart,
    MoveToPoint(Point2<Pixels>),
    MoveToTextEnd,
    MoveToTextStart,
    MoveUp,
    SelectAll,
    SelectDown,
    SelectLeft,
    SelectWordLeft,
    SelectLineAtPoint(Point2<Pixels>),
    SelectRight,
    SelectWordRight,
    SelectToLineEnd,
    SelectToLineStart,
    SelectToTextEnd,
    SelectToTextStart,
    SelectUp,
    SelectWordAtPoint(Point2<Pixels>),
}

pub struct TextEditorState<T: EditableTextBuffer> {
    content: Rc<TextEditorContent<T>>,
}

impl<T: EditableTextBuffer> TextEditorState<T> {
    pub fn new(buffer: T) -> Self {
        Self {
            content: Rc::new(TextEditorContent {
                buffer: RefCell::new(buffer),
                applied_style: Cell::new(None),
                #[cfg(debug_assertions)]
                app_frame_last_used: Cell::new(None),
            }),
        }
    }

    pub fn with_buffer<R>(&self, callback: impl FnOnce(&T) -> R) -> R {
        let buffer = self.content.buffer.borrow();
        callback(&buffer)
    }

    pub fn with_buffer_mut<R>(&self, callback: impl FnOnce(&mut T) -> R) -> R {
        let mut buffer = self.content.buffer.borrow_mut();
        callback(&mut buffer)
    }
}

pub type PlainTextEditorState = TextEditorState<PlainTextBuffer>;

impl TextEditorState<PlainTextBuffer> {
    pub fn plain() -> Self {
        Self::new(PlainTextBuffer::default())
    }

    pub fn set_text(&self, text: &str) {
        self.with_buffer_mut(|buffer| buffer.editor.set_text(text));
    }

    pub fn with_raw_text<R>(&self, callback: impl FnOnce(&str) -> R) -> R {
        self.with_buffer(|buffer| callback(buffer.editor.raw_text()))
    }

    pub fn is_composing(&self) -> bool {
        self.with_buffer(|buffer| buffer.editor.is_composing())
    }
}

struct TextEditorContent<T: EditableTextBuffer> {
    buffer: RefCell<T>,
    // The (theme revision, state flags) the buffer styles were last resolved
    // from. Reapplying styles marks the text layout dirty, so it must be
    // skipped when nothing changed.
    applied_style: Cell<Option<(u64, StateFlags)>>,
    #[cfg(debug_assertions)]
    app_frame_last_used: Cell<Option<u64>>,
}

impl<T: EditableTextBuffer> TextEditorContent<T> {
    fn check_frame_use(&self, app_frame_counter: u64) {
        #[cfg(debug_assertions)]
        {
            let last_frame_used = self.app_frame_last_used.get();
            assert_ne!(
                last_frame_used,
                Some(app_frame_counter),
                "TextEditorState cannot be rendered more than once in the same frame"
            );
            self.app_frame_last_used.set(Some(app_frame_counter));
        }

        #[cfg(not(debug_assertions))]
        {
            let _ = app_frame_counter;
        }
    }
}

impl<T: EditableTextBuffer> EditableTextContent for TextEditorContent<T> {
    fn measure(
        &self,
        text_context: &mut TextLayoutContext,
        max_width: f32,
        alignment: TextAlignment,
    ) -> Option<f32> {
        self.buffer
            .borrow_mut()
            .measure(text_context, max_width, alignment)
    }

    fn draw(
        &self,
        text_context: &mut TextLayoutContext,
        canvas: &mut Canvas,
        layout: &NodeLayout,
        visuals: EditableTextVisuals,
    ) {
        let mut buffer = self.buffer.borrow_mut();
        let clip = layout.effective_clip;

        buffer.with_layouts(text_context, |text_layout| {
            let offset = text_layout.offset();
            let x = layout.x + offset.x;
            let y = layout.y + offset.y;

            text_layout.selection_geometry_with(|bbox, _| {
                fill_snapped_rect(canvas, &bbox, visuals.selection_color, x, y, clip);
            });

            if let Some(mut rect) = text_layout.cursor_geometry(visuals.cursor_size) {
                // Draw the caret as a 2px-wide bar regardless of the width
                // reported by the layout.
                rect.size.width = 2.0;
                fill_snapped_rect(canvas, &rect, visuals.cursor_color, x, y, clip);
            }

            canvas.draw_text_layout(text_layout.layout(), [x, y], clip);
        });
    }
}

pub struct TextEdit<'a, T: EditableTextBuffer> {
    builder: UiBuilder<'a>,
    interaction: Interaction,
    state_flags: StateFlags,
    state: &'a TextEditorState<T>,
}

impl<'a, T: EditableTextBuffer + 'static> TextEdit<'a, T> {
    pub fn new(builder: &'a mut UiBuilder<'_>, state: &'a TextEditorState<T>) -> Self {
        let mut builder = builder.child();

        let (interaction, state_flags) = Interaction::compute(
            &builder,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED | StateFlags::FOCUSED,
        );

        builder.set_active(state_flags.contains(StateFlags::PRESSED));

        // Apply styles early as defaults, so that users have opportunity to
        // override them before calling `finish()`.
        builder.apply_style(StyleClass::TextEdit, state_flags);

        let min_height = {
            let style = builder.theme.get(StyleClass::TextEdit);

            let text_height = style.font_size.get(state_flags) as f32;
            let padding = style.padding.get(state_flags);

            text_height + padding.top + padding.bottom
        };

        builder.size(
            Size::Fit {
                min: DEFAULT_TEXT_EDIT_WIDTH,
                max: f32::MAX,
            },
            Size::Fit {
                min: min_height,
                max: f32::MAX,
            },
        );

        Self {
            builder,
            interaction,
            state_flags,
            state,
        }
    }

    forward_properties!(width, height);

    pub fn default_text(self, text: &str) -> Self {
        let mut buffer = self.state.content.buffer.borrow_mut();

        if buffer.is_empty() {
            buffer.enter_text(self.builder.text_context, text);
        }

        self
    }

    pub fn text(mut self, text: &str) -> Self {
        self.set_text(text);
        self
    }

    pub fn set_text(&mut self, text: &str) -> &mut Self {
        let mut buffer = self.state.content.buffer.borrow_mut();

        buffer.move_cursor(self.builder.text_context, TextEditMotion::SelectAll);
        buffer.enter_text(self.builder.text_context, text);

        self
    }

    pub fn paint(
        &mut self,
        paint: Paint,
        border: GradientPaint,
        border_width: BorderWidths,
        corner_radii: CornerRadii,
    ) -> &mut Self {
        self.builder
            .paint(paint, border, border_width, corner_radii);
        self
    }

    pub fn finish(mut self) -> Interaction {
        self.state
            .content
            .check_frame_use(self.builder.context.app_frame_counter);

        let is_focused = self.state_flags.contains(StateFlags::FOCUSED);

        if is_focused {
            self.builder.request_focus();
        } else {
            self.builder.release_focus();
        }

        let theme = self.builder.theme;

        let input = self.builder.input().clone();
        let placement = self.builder.prev_state().map(|s| s.placement);

        let mut buffer = self.state.content.buffer.borrow_mut();
        let style = theme.get(StyleClass::TextEdit);
        let padding = style.padding.get(self.state_flags);

        let style_key = (theme.revision(), self.state_flags);
        if self.state.content.applied_style.get() != Some(style_key) {
            self.state.content.applied_style.set(Some(style_key));
            buffer.apply_style(style, self.state_flags);
        }

        if let Some(placement) = placement {
            self.handle_mouse_events(
                &mut buffer,
                &input,
                placement,
                padding,
                self.interaction,
                self.state_flags,
            );
        }

        if is_focused {
            self.handle_keyboard_events(&mut buffer, &input);
        }

        let cursor_size = style.font_size.get(self.state_flags) as f32;

        let (selection_color, cursor_color) = if is_focused {
            let sel_color = style.selection_color.get(self.state_flags);
            let cur_color = style.cursor_color.get(self.state_flags);
            (sel_color, cur_color)
        } else {
            Default::default()
        };

        let visuals = EditableTextVisuals {
            alignment: style.text_align.get(self.state_flags),
            cursor_size,
            selection_color,
            cursor_color,
        };

        drop(buffer);

        let content: Rc<dyn EditableTextContent> = self.state.content.clone();

        self.builder.context.ui_tree.add(
            Some(self.builder.index),
            Atom {
                width: Size::Grow,
                height: Size::Fit {
                    min: cursor_size,
                    max: f32::MAX,
                },
                ..Default::default()
            },
            (LayoutContent::EditableText { content, visuals }, None),
        );

        self.interaction
    }

    fn handle_keyboard_events(&mut self, buffer: &mut T, input: &Input) {
        for event in input.keyboard_events.iter() {
            if !event.state.is_pressed() {
                continue;
            }

            let ctrl_held = input.modifiers.control_key();
            let shift_held = input.modifiers.shift_key();

            let motion = match event.key {
                PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight) => continue,
                PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => continue,
                PhysicalKey::Code(KeyCode::KeyA) if ctrl_held => TextEditMotion::SelectAll,
                PhysicalKey::Code(KeyCode::ArrowLeft) => match (ctrl_held, shift_held) {
                    (true, true) => TextEditMotion::SelectWordLeft,
                    (true, false) => TextEditMotion::MoveWordLeft,
                    (false, true) => TextEditMotion::SelectLeft,
                    (false, false) => TextEditMotion::MoveLeft,
                },
                PhysicalKey::Code(KeyCode::ArrowRight) => match (ctrl_held, shift_held) {
                    (true, true) => TextEditMotion::SelectWordRight,
                    (true, false) => TextEditMotion::MoveWordRight,
                    (false, true) => TextEditMotion::SelectRight,
                    (false, false) => TextEditMotion::MoveRight,
                },
                PhysicalKey::Code(KeyCode::ArrowUp) => match shift_held {
                    true => TextEditMotion::SelectUp,
                    false => TextEditMotion::MoveUp,
                },
                PhysicalKey::Code(KeyCode::ArrowDown) => match shift_held {
                    true => TextEditMotion::SelectDown,
                    false => TextEditMotion::MoveDown,
                },
                PhysicalKey::Code(KeyCode::PageUp) => match shift_held {
                    true => TextEditMotion::SelectUp,
                    false => TextEditMotion::MoveUp,
                },
                PhysicalKey::Code(KeyCode::PageDown) => match shift_held {
                    true => TextEditMotion::SelectDown,
                    false => TextEditMotion::MoveDown,
                },
                PhysicalKey::Code(KeyCode::Backspace) => match ctrl_held {
                    true => TextEditMotion::BackdeleteWord,
                    false => TextEditMotion::Backdelete,
                },
                PhysicalKey::Code(KeyCode::Delete) => match ctrl_held {
                    true => TextEditMotion::DeleteWord,
                    false => TextEditMotion::Delete,
                },
                PhysicalKey::Code(KeyCode::Home) => match (ctrl_held, shift_held) {
                    (true, true) => TextEditMotion::SelectToTextStart,
                    (true, false) => TextEditMotion::MoveToTextStart,
                    (false, true) => TextEditMotion::SelectToLineStart,
                    (false, false) => TextEditMotion::MoveToLineStart,
                },
                PhysicalKey::Code(KeyCode::End) => match (ctrl_held, shift_held) {
                    (true, true) => TextEditMotion::SelectToTextEnd,
                    (true, false) => TextEditMotion::MoveToTextEnd,
                    (false, true) => TextEditMotion::SelectToLineEnd,
                    (false, false) => TextEditMotion::MoveToLineEnd,
                },
                PhysicalKey::Code(KeyCode::KeyC) if ctrl_held => {
                    if let Some(text) = buffer.selected_text() {
                        self.builder.clipboard.set_text(text);
                    }

                    continue;
                }
                PhysicalKey::Code(KeyCode::KeyV) if ctrl_held => {
                    if let Some(text) = self.builder.clipboard.get_text() {
                        buffer.enter_text(self.builder.text_context, &text);
                    }

                    continue;
                }
                _ => {
                    if let Some(text) = &event.text {
                        buffer.enter_text(self.builder.text_context, text);
                    }

                    continue;
                }
            };

            buffer.move_cursor(self.builder.text_context, motion);
        }
    }

    fn handle_mouse_events(
        &mut self,
        buffer: &mut T,
        input: &Input,
        placement: glamour::Rect<crate::ui::Pixels>,
        padding: crate::ui::Padding,
        interaction: Interaction,
        state_flags: StateFlags,
    ) {
        let left_click_count = input.mouse_state.left_click_count;
        let is_hovered = state_flags.contains(StateFlags::HOVERED);
        let is_focused = state_flags.contains(StateFlags::FOCUSED);

        if !(is_hovered || is_focused) {
            return;
        }

        let content_min = Point2::new(
            placement.origin.x + padding.left,
            placement.origin.y + padding.top,
        );
        let content_max = Point2::new(
            (placement.origin.x + placement.size.width - padding.right).max(content_min.x),
            (placement.origin.y + placement.size.height - padding.bottom).max(content_min.y),
        );
        let clamped = input.pointer.clamp(content_min, content_max);
        let local = clamped - content_min;

        let is_shift_held = input.modifiers.shift_key();
        let is_left_down = input.mouse_state.is_left_down();
        let local = local.to_point();

        let motion = if interaction.is_activated && is_hovered {
            match left_click_count {
                4.. => Some(TextEditMotion::SelectAll),
                3 => Some(TextEditMotion::SelectLineAtPoint(local)),
                2 => Some(TextEditMotion::SelectWordAtPoint(local)),
                1 if is_shift_held => Some(TextEditMotion::ExtendSelectionToPoint(local)),
                1 => Some(TextEditMotion::MoveToPoint(local)),
                0 => None,
            }
        } else if is_left_down && is_focused {
            Some(match left_click_count {
                3 => TextEditMotion::SelectLineAtPoint(local),
                2 => TextEditMotion::SelectWordAtPoint(local),
                _ => TextEditMotion::ExtendSelectionToPoint(local),
            })
        } else {
            None
        };

        if let Some(motion) = motion {
            buffer.move_cursor(self.builder.text_context, motion);
        }
    }
}

/// Fill a rect from a text layout, snapping its vertical extent to whole
/// pixels.
fn fill_snapped_rect(
    canvas: &mut Canvas,
    rect: &Rect<Pixels>,
    color: Color,
    x: f32,
    y: f32,
    clip: ClipRect,
) {
    let y0 = (y + rect.origin.y).round();
    let y1 = (y + rect.origin.y + rect.size.height).round();

    canvas.draw(Primitive {
        point: [x + rect.origin.x, y0],
        size: [rect.size.width, y1 - y0],
        clip,
        paint: Paint::solid(color),
        border: GradientPaint::default(),
        border_width: [0.0; 4],
        corner_radii: [0.0; 4],
        use_nearest_sampling: false,
    });
}

pub struct PlainEditorTextLayout<'a> {
    editor: &'a PlainEditor<Color>,
}

impl EditableTextLayout for PlainEditorTextLayout<'_> {
    fn layout(&self) -> &parley::Layout<Color> {
        self.editor.try_layout().unwrap()
    }

    fn selection_geometry_with(&self, mut callback: impl FnMut(Rect<Pixels>, usize)) {
        self.editor
            .selection_geometry_with(|bbox, line| callback(bounding_box_rect(bbox), line));
    }

    fn cursor_geometry(&self, cursor_size: f32) -> Option<Rect<Pixels>> {
        self.editor
            .cursor_geometry(cursor_size)
            .map(bounding_box_rect)
    }
}

/// An [`EditableTextBuffer`] backed by parley's single-style [`PlainEditor`].
pub struct PlainTextBuffer {
    editor: PlainEditor<Color>,
    // The width/alignment last pushed to the editor. Setting either marks the
    // parley layout dirty even when the value is unchanged, so only real
    // changes are forwarded.
    prev_width: Option<f32>,
    prev_alignment: Option<TextAlignment>,
}

impl Default for PlainTextBuffer {
    fn default() -> Self {
        Self {
            editor: PlainEditor::new(14.0),
            prev_width: None,
            prev_alignment: None,
        }
    }
}

impl EditableTextBuffer for PlainTextBuffer {
    type Layout<'a> = PlainEditorTextLayout<'a>;

    fn is_empty(&self) -> bool {
        self.editor.raw_text().is_empty()
    }

    fn selected_text(&self) -> Option<&str> {
        self.editor.selected_text()
    }

    fn apply_style(&mut self, style: &Style, state: StateFlags) {
        let styles = self.editor.edit_styles();

        enumerate_text_style(style, state, |prop| {
            styles.insert(prop);
        });
    }

    fn enter_text(&mut self, context: &mut TextLayoutContext, text: &str) {
        context
            .drive(&mut self.editor)
            .insert_or_replace_selection(text);
    }

    fn move_cursor(&mut self, context: &mut TextLayoutContext, motion: TextEditMotion) {
        let mut driver = context.drive(&mut self.editor);

        match motion {
            TextEditMotion::Backdelete => driver.backdelete(),
            TextEditMotion::BackdeleteWord => driver.backdelete_word(),
            TextEditMotion::Delete => driver.delete(),
            TextEditMotion::DeleteWord => driver.delete_word(),
            TextEditMotion::ExtendSelectionToPoint(p) => {
                driver.extend_selection_to_point(p.x, p.y);
            }
            TextEditMotion::MoveDown => driver.move_down(),
            TextEditMotion::MoveLeft => driver.move_left(),
            TextEditMotion::MoveWordLeft => driver.move_word_left(),
            TextEditMotion::MoveRight => driver.move_right(),
            TextEditMotion::MoveWordRight => driver.move_word_right(),
            TextEditMotion::MoveToLineEnd => driver.move_to_line_end(),
            TextEditMotion::MoveToLineStart => driver.move_to_line_start(),
            TextEditMotion::MoveToPoint(p) => driver.move_to_point(p.x, p.y),
            TextEditMotion::MoveToTextEnd => driver.move_to_text_end(),
            TextEditMotion::MoveToTextStart => driver.move_to_text_start(),
            TextEditMotion::MoveUp => driver.move_up(),
            TextEditMotion::SelectAll => driver.select_all(),
            TextEditMotion::SelectDown => driver.select_down(),
            TextEditMotion::SelectLeft => driver.select_left(),
            TextEditMotion::SelectLineAtPoint(p) => {
                driver.select_line_at_point(p.x, p.y);
            }
            TextEditMotion::SelectRight => driver.select_right(),
            TextEditMotion::SelectUp => driver.select_up(),
            TextEditMotion::SelectWordAtPoint(p) => {
                driver.select_word_at_point(p.x, p.y);
            }
            TextEditMotion::SelectWordLeft => driver.select_word_left(),
            TextEditMotion::SelectWordRight => driver.select_word_right(),
            TextEditMotion::SelectToLineEnd => driver.select_to_line_end(),
            TextEditMotion::SelectToLineStart => driver.select_to_line_start(),
            TextEditMotion::SelectToTextEnd => driver.select_to_text_end(),
            TextEditMotion::SelectToTextStart => driver.select_to_text_start(),
        }
    }

    fn measure(
        &mut self,
        context: &mut TextLayoutContext,
        max_width: f32,
        alignment: TextAlignment,
    ) -> Option<f32> {
        if self.prev_width != Some(max_width) {
            self.prev_width = Some(max_width);
            self.editor.set_width(Some(max_width));
        }

        if self.prev_alignment != Some(alignment) {
            self.prev_alignment = Some(alignment);
            self.editor.set_alignment(alignment.into());
        }

        Some(
            self.editor
                .layout(&mut context.fonts, &mut context.layouts)
                .height(),
        )
    }

    fn with_layouts<'a>(
        &'a mut self,
        context: &'a mut TextLayoutContext,
        mut callback: impl FnMut(Self::Layout<'a>),
    ) {
        self.editor
            .refresh_layout(&mut context.fonts, &mut context.layouts);
        callback(PlainEditorTextLayout {
            editor: &self.editor,
        });
    }
}

fn bounding_box_rect(bbox: parley::BoundingBox) -> Rect<Pixels> {
    Rect {
        origin: Point2 {
            x: bbox.x0 as f32,
            y: bbox.y0 as f32,
        },
        size: Size2 {
            width: (bbox.x1 - bbox.x0) as f32,
            height: (bbox.y1 - bbox.y0) as f32,
        },
    }
}
