use parley::PlainEditor;
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

use crate::graphics::Color;
use crate::graphics::GradientPaint;
use crate::graphics::Paint;
use crate::graphics::TextLayoutContext;
use crate::shell::Clipboard;
use crate::shell::Input;
use crate::ui::Atom;
use crate::ui::Size;
use crate::ui::builder::UiBuilder;
use crate::ui::context::LayoutContent;
use crate::ui::style::BorderWidths;
use crate::ui::style::CornerRadii;
use crate::ui::style::StateFlags;
use crate::ui::theme::StyleClass;
use crate::ui::widget::ClickBehavior;
use crate::ui::widget::Interaction;

use super::macros::forward_properties;

pub struct TextEdit<'a> {
    builder: UiBuilder<'a>,
    interaction: Interaction,
    state_flags: StateFlags,
}

impl<'a> TextEdit<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>, width: Size) -> Self {
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
            width,
            Size::Fit {
                min: min_height,
                max: f32::MAX,
            },
        );

        Self {
            builder,
            interaction,
            state_flags,
        }
    }

    forward_properties!(width, height);

    pub fn default_text(self, text: &str) -> Self {
        let (_, dynamic_layout) = self
            .builder
            .context
            .dynamic_text_layout(self.builder.text_layouts, self.builder.id);

        if dynamic_layout.editor.raw_text().is_empty() {
            dynamic_layout.editor.set_text(text);
        }

        self
    }

    pub fn text(self, text: &str) -> Self {
        let (_, dynamic_layout) = self
            .builder
            .context
            .dynamic_text_layout(self.builder.text_layouts, self.builder.id);

        dynamic_layout.editor.set_text(text);

        self
    }

    pub fn set_text(&mut self, text: &str) -> &mut Self {
        let (_, dynamic_layout) = self
            .builder
            .context
            .dynamic_text_layout(self.builder.text_layouts, self.builder.id);

        dynamic_layout.editor.set_text(text);

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

    pub fn finish(mut self) -> (Option<&'a str>, Interaction) {
        let is_focused = self.state_flags.contains(StateFlags::FOCUSED);

        if is_focused {
            self.builder.request_focus();
        } else {
            self.builder.release_focus();
        }

        let style_id = self.builder.theme().get_id(StyleClass::TextEdit);
        let theme = self.builder.theme;

        let input = self.builder.input().clone();
        let (placement, has_text_layout) = self
            .builder
            .prev_state()
            .map(|s| (s.placement, s.text_layout.is_some()))
            .unwrap_or_default();

        let (text_layout_id, dynamic_layout) = self
            .builder
            .context
            .dynamic_text_layout(self.builder.text_layouts, self.builder.id);

        let style =
            theme.apply_plain_editor_styles(style_id, self.state_flags, &mut dynamic_layout.editor);

        if has_text_layout {
            Self::handle_mouse_events(
                &mut dynamic_layout.editor.driver(
                    &mut self.builder.text_context.fonts,
                    &mut self.builder.text_context.layouts,
                ),
                &input,
                placement,
                self.interaction,
                self.state_flags,
            );
        }

        if is_focused {
            Self::handle_keyboard_events(
                self.builder.clipboard,
                self.builder.text_context,
                &mut dynamic_layout.editor,
                &input,
            );
        }

        let cursor_size = style.font_size.get(self.state_flags) as f32;

        let (selection_color, cursor_color) = if is_focused {
            let sel_color = style.selection_color.get(self.state_flags);
            let cur_color = style.cursor_color.get(self.state_flags);
            (sel_color, cur_color)
        } else {
            Default::default()
        };

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
            (
                LayoutContent::Text {
                    layout: text_layout_id,
                    alignment: style.text_align.get(self.state_flags),
                    cursor_size,
                    selection_color,
                    cursor_color,
                },
                None,
            ),
        );

        let is_composing = dynamic_layout.editor.is_composing();
        let text = (!is_composing).then_some(dynamic_layout.editor.raw_text());

        (text, self.interaction)
    }

    fn handle_keyboard_events(
        clipboard: &mut Clipboard,
        context: &mut TextLayoutContext,
        editor: &mut PlainEditor<Color>,
        input: &Input,
    ) {
        macro_rules! driver {
            () => {
                context.drive(editor)
            };
        }

        for event in input.keyboard_events.iter() {
            if !event.state.is_pressed() {
                continue;
            }

            let ctrl_held = input.modifiers.control_key();
            let shift_held = input.modifiers.shift_key();

            match event.key {
                PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight) => {}
                PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => {}
                PhysicalKey::Code(KeyCode::KeyA) if ctrl_held => {
                    driver!().select_all();
                }
                PhysicalKey::Code(KeyCode::ArrowLeft) => match (ctrl_held, shift_held) {
                    (true, true) => driver!().select_word_left(),
                    (true, false) => driver!().move_word_left(),
                    (false, true) => driver!().select_left(),
                    (false, false) => driver!().move_left(),
                },
                PhysicalKey::Code(KeyCode::ArrowRight) => match (ctrl_held, shift_held) {
                    (true, true) => driver!().select_word_right(),
                    (true, false) => driver!().move_word_right(),
                    (false, true) => driver!().select_right(),
                    (false, false) => driver!().move_right(),
                },
                PhysicalKey::Code(KeyCode::ArrowUp) => match shift_held {
                    true => driver!().select_up(),
                    false => driver!().move_up(),
                },
                PhysicalKey::Code(KeyCode::ArrowDown) => match shift_held {
                    true => driver!().select_down(),
                    false => driver!().move_down(),
                },
                PhysicalKey::Code(KeyCode::PageUp) => match shift_held {
                    true => driver!().select_up(),
                    false => driver!().move_up(),
                },
                PhysicalKey::Code(KeyCode::PageDown) => match shift_held {
                    true => driver!().select_down(),
                    false => driver!().move_down(),
                },
                PhysicalKey::Code(KeyCode::Backspace) => match ctrl_held {
                    true => driver!().backdelete_word(),
                    false => driver!().backdelete(),
                },
                PhysicalKey::Code(KeyCode::Delete) => match ctrl_held {
                    true => driver!().delete_word(),
                    false => driver!().delete(),
                },
                PhysicalKey::Code(KeyCode::Home) => match (ctrl_held, shift_held) {
                    (true, true) => driver!().select_to_text_start(),
                    (true, false) => driver!().move_to_text_start(),
                    (false, true) => driver!().select_to_line_start(),
                    (false, false) => driver!().move_to_line_start(),
                },
                PhysicalKey::Code(KeyCode::End) => match (ctrl_held, shift_held) {
                    (true, true) => driver!().select_to_text_end(),
                    (true, false) => driver!().move_to_text_end(),
                    (false, true) => driver!().select_to_line_end(),
                    (false, false) => driver!().move_to_line_end(),
                },
                PhysicalKey::Code(KeyCode::KeyC) if ctrl_held => {
                    if let Some(text) = editor.selected_text() {
                        clipboard.set_text(text);
                    }
                }
                PhysicalKey::Code(KeyCode::KeyV) if ctrl_held => {
                    if let Some(text) = clipboard.get_text() {
                        driver!().insert_or_replace_selection(&text);
                    }
                }
                _ => {
                    if let Some(text) = &event.text {
                        driver!().insert_or_replace_selection(text.as_str());
                    }
                }
            }
        }
    }

    fn handle_mouse_events(
        driver: &mut parley::PlainEditorDriver<Color>,
        input: &Input,
        placement: glamour::Rect<crate::ui::Pixels>,
        interaction: Interaction,
        state_flags: StateFlags,
    ) {
        let left_click_count = input.mouse_state.left_click_count;
        let is_hovered = state_flags.contains(StateFlags::HOVERED);
        let is_focused = state_flags.contains(StateFlags::FOCUSED);

        if !(is_hovered || is_focused) {
            return;
        }

        let max = placement.origin + placement.size.to_vector();
        let clamped = input.pointer.clamp(placement.origin, max);
        let local = clamped - placement.origin;

        let is_shift_held = input.modifiers.shift_key();
        let is_left_down = input.mouse_state.is_left_down();

        if interaction.is_activated && is_hovered {
            match left_click_count {
                4.. => driver.select_all(),
                3 => driver.select_line_at_point(local.x, local.y),
                2 => driver.select_word_at_point(local.x, local.y),
                1 if is_shift_held => driver.extend_selection_to_point(local.x, local.y),
                1 => driver.move_to_point(local.x, local.y),
                0 => {}
            }
        } else if is_left_down && is_focused {
            match left_click_count {
                3 => driver.select_line_at_point(local.x, local.y),
                2 => driver.select_word_at_point(local.x, local.y),
                _ => driver.extend_selection_to_point(local.x, local.y),
            }
        }
    }
}
