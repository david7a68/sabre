use std::cell::RefCell;
use std::rc::Rc;

use parley::PlainEditor;

use crate::graphics::Canvas;
use crate::graphics::ClipRect;
use crate::graphics::Color;
use crate::graphics::GradientPaint;
use crate::graphics::Paint;
use crate::graphics::Primitive;
use crate::graphics::TextAlignment;

use super::super::context::LayoutRect;
use super::super::context::UiContent;
use super::super::style::StateFlags;
use super::super::style::StyleId;
use super::super::theme::Theme;
use super::EditableText;
use super::EditableTextHandle;
use super::EditableTextState;
use super::TextServices;

#[derive(Clone)]
pub struct PlainEditableText {
    inner: EditableTextHandle,
}

impl PlainEditableText {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(PlainEditableTextInner {
                editor: PlainEditor::new(14.0),
                prev_width: 0.0,
                prev_alignment: None,
            })),
        }
    }

    pub fn raw_text(&self) -> String {
        self.inner.borrow().raw_text().to_owned()
    }

    pub fn set_text(&self, text: &str) {
        self.inner.borrow_mut().set_text(text);
    }

    pub fn selected_text(&self) -> Option<String> {
        self.inner.borrow().selected_text().map(str::to_owned)
    }

    pub fn is_composing(&self) -> bool {
        self.inner.borrow().is_composing()
    }
}

impl Default for PlainEditableText {
    fn default() -> Self {
        Self::new()
    }
}

impl EditableText for PlainEditableText {
    fn handle(&self) -> EditableTextHandle {
        self.inner.clone()
    }
}

struct PlainEditableTextInner {
    editor: PlainEditor<Color>,
    prev_width: f32,
    prev_alignment: Option<TextAlignment>,
}

impl EditableTextState for PlainEditableTextInner {
    fn raw_text(&self) -> &str {
        self.editor.raw_text()
    }

    fn set_text(&mut self, text: &str) {
        self.editor.set_text(text);
    }

    fn selected_text(&self) -> Option<&str> {
        self.editor.selected_text()
    }

    fn is_composing(&self) -> bool {
        self.editor.is_composing()
    }

    fn apply_style(
        &mut self,
        theme: &Theme,
        style_id: StyleId,
        state: StateFlags,
    ) -> TextEditPaint {
        let style = theme.apply_plain_editor_styles(style_id, state, &mut self.editor);
        TextEditPaint {
            alignment: style.text_align.get(state),
            cursor_size: style.font_size.get(state) as f32,
            selection_color: style.selection_color.get(state),
            cursor_color: style.cursor_color.get(state),
        }
    }

    fn command(&mut self, services: &TextServices, command: TextEditCommand<'_>) {
        services.with_context(|context| {
            let mut driver = context.drive(&mut self.editor);

            match command {
                TextEditCommand::SelectAll => driver.select_all(),
                TextEditCommand::MoveLeft { select, word } => match (select, word) {
                    (true, true) => driver.select_word_left(),
                    (true, false) => driver.select_left(),
                    (false, true) => driver.move_word_left(),
                    (false, false) => driver.move_left(),
                },
                TextEditCommand::MoveRight { select, word } => match (select, word) {
                    (true, true) => driver.select_word_right(),
                    (true, false) => driver.select_right(),
                    (false, true) => driver.move_word_right(),
                    (false, false) => driver.move_right(),
                },
                TextEditCommand::MoveUp { select } => match select {
                    true => driver.select_up(),
                    false => driver.move_up(),
                },
                TextEditCommand::MoveDown { select } => match select {
                    true => driver.select_down(),
                    false => driver.move_down(),
                },
                TextEditCommand::MoveToLineStart { select } => match select {
                    true => driver.select_to_line_start(),
                    false => driver.move_to_line_start(),
                },
                TextEditCommand::MoveToLineEnd { select } => match select {
                    true => driver.select_to_line_end(),
                    false => driver.move_to_line_end(),
                },
                TextEditCommand::MoveToTextStart { select } => match select {
                    true => driver.select_to_text_start(),
                    false => driver.move_to_text_start(),
                },
                TextEditCommand::MoveToTextEnd { select } => match select {
                    true => driver.select_to_text_end(),
                    false => driver.move_to_text_end(),
                },
                TextEditCommand::Backdelete { word } => match word {
                    true => driver.backdelete_word(),
                    false => driver.backdelete(),
                },
                TextEditCommand::Delete { word } => match word {
                    true => driver.delete_word(),
                    false => driver.delete(),
                },
                TextEditCommand::InsertOrReplaceSelection(text) => {
                    driver.insert_or_replace_selection(text);
                }
                TextEditCommand::MoveToPoint { x, y } => driver.move_to_point(x, y),
                TextEditCommand::ExtendSelectionToPoint { x, y } => {
                    driver.extend_selection_to_point(x, y);
                }
                TextEditCommand::SelectWordAtPoint { x, y } => driver.select_word_at_point(x, y),
                TextEditCommand::SelectLineAtPoint { x, y } => driver.select_line_at_point(x, y),
            }
        });
    }

    fn measure(
        &mut self,
        services: &TextServices,
        max_width: f32,
        alignment: TextAlignment,
    ) -> f32 {
        let width_changed = self.prev_width != max_width;
        if width_changed {
            self.editor.set_width(Some(max_width));
        }

        let alignment_changed = self.prev_alignment != Some(alignment);
        if alignment_changed {
            self.editor.set_alignment(alignment.into());
        }

        self.prev_width = max_width;
        self.prev_alignment = Some(alignment);

        services.with_context(|context| {
            self.editor
                .layout(&mut context.fonts, &mut context.layouts)
                .height()
        })
    }

    fn draw(
        &mut self,
        services: &TextServices,
        canvas: &mut Canvas,
        rect: LayoutRect,
        clip: ClipRect,
        paint: TextEditPaint,
    ) {
        let x = rect.origin.x;
        let y = rect.origin.y;

        self.editor.selection_geometry_with(|bbox, _| {
            draw_selection_rect(canvas, &bbox, paint.selection_color, x, y, clip);
        });

        if let Some(rect) = self.editor.cursor_geometry(paint.cursor_size) {
            draw_cursor(canvas, &rect, paint.cursor_color, x, y, clip);
        }

        services.with_context(|context| {
            canvas.draw_text_layout(
                self.editor.layout(&mut context.fonts, &mut context.layouts),
                [x, y],
                clip,
            );
        });
    }
}

#[derive(Clone, Copy)]
pub struct TextEditPaint {
    pub alignment: TextAlignment,
    pub cursor_size: f32,
    pub selection_color: Color,
    pub cursor_color: Color,
}

pub enum TextEditCommand<'a> {
    SelectAll,
    MoveLeft { select: bool, word: bool },
    MoveRight { select: bool, word: bool },
    MoveUp { select: bool },
    MoveDown { select: bool },
    MoveToLineStart { select: bool },
    MoveToLineEnd { select: bool },
    MoveToTextStart { select: bool },
    MoveToTextEnd { select: bool },
    Backdelete { word: bool },
    Delete { word: bool },
    InsertOrReplaceSelection(&'a str),
    MoveToPoint { x: f32, y: f32 },
    ExtendSelectionToPoint { x: f32, y: f32 },
    SelectWordAtPoint { x: f32, y: f32 },
    SelectLineAtPoint { x: f32, y: f32 },
}

pub(crate) struct EditableTextContent {
    services: TextServices,
    text: EditableTextHandle,
    paint: TextEditPaint,
}

impl EditableTextContent {
    pub(crate) fn new(
        services: TextServices,
        text: EditableTextHandle,
        paint: TextEditPaint,
    ) -> Rc<Self> {
        Rc::new(Self {
            services,
            text,
            paint,
        })
    }
}

impl UiContent for EditableTextContent {
    fn measure(&self, max_width: f32) -> Option<f32> {
        Some(
            self.text
                .borrow_mut()
                .measure(&self.services, max_width, self.paint.alignment),
        )
    }

    fn draw(&self, canvas: &mut Canvas, rect: LayoutRect, clip: ClipRect) {
        self.text
            .borrow_mut()
            .draw(&self.services, canvas, rect, clip, self.paint);
    }
}

fn draw_selection_rect(
    canvas: &mut Canvas,
    rect: &parley::BoundingBox,
    color: Color,
    x: f32,
    y: f32,
    clip: ClipRect,
) {
    let y0 = (y + rect.y0 as f32).round();
    let y1 = (y + rect.y1 as f32).round();

    canvas.draw(Primitive {
        point: [x + rect.x0 as f32, y0],
        size: [(rect.x1 - rect.x0) as f32, y1 - y0],
        clip,
        paint: Paint::solid(color),
        border: GradientPaint::default(),
        border_width: [0.0; 4],
        corner_radii: [0.0; 4],
        use_nearest_sampling: false,
    });
}

fn draw_cursor(
    canvas: &mut Canvas,
    cursor_rect: &parley::BoundingBox,
    color: Color,
    x: f32,
    y: f32,
    clip: ClipRect,
) {
    let y0 = (y + cursor_rect.y0 as f32).round();
    let y1 = (y + cursor_rect.y1 as f32).round();

    canvas.draw(Primitive {
        point: [x + cursor_rect.x0 as f32, y0],
        size: [2.0, y1 - y0],
        clip,
        paint: Paint::solid(color),
        border: GradientPaint::default(),
        border_width: [0.0; 4],
        corner_radii: [0.0; 4],
        use_nearest_sampling: false,
    });
}
