use parley::Cursor;
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

use crate::ui::Atom;
use crate::ui::Size;
use crate::ui::builder::UiBuilder;
use crate::ui::context::LayoutContent;
use crate::ui::style::StateFlags;
use crate::ui::theme::StyleClass;
use crate::ui::widget::ClickBehavior;
use crate::ui::widget::Interaction;

pub struct TextEdit<'a> {
    builder: UiBuilder<'a>,
    interaction: Interaction,
    state_flags: StateFlags,
    wrap_text: bool,
}

impl<'a> TextEdit<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>, width: Size) -> Self {
        let mut builder = builder.child();
        builder.width(width);
        builder.height(Size::Fixed(120.0));

        let (interaction, state_flags) = Interaction::compute(
            &builder,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED | StateFlags::FOCUSED,
        );

        Self {
            builder,
            interaction,
            state_flags,
            wrap_text: false,
        }
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap_text = wrap;
        self
    }

    pub fn default_text(self, text: &str) -> Self {
        let (_, dynamic_layout) = self
            .builder
            .context
            .upsert_dynamic_text_layout(self.builder.id);

        if dynamic_layout.editor.raw_text().is_empty() {
            dynamic_layout.editor.set_text(text);
        }

        self
    }

    pub fn text(self, text: &str) -> Self {
        let (_, dynamic_layout) = self
            .builder
            .context
            .upsert_dynamic_text_layout(self.builder.id);

        dynamic_layout.editor.set_text(text);

        self
    }

    pub fn finish(mut self) -> (Option<&'a str>, Interaction) {
        let is_focused = self.state_flags.contains(StateFlags::FOCUSED);

        if is_focused {
            self.builder.request_focus();
        } else {
            self.builder.release_focus();
        }

        let style_id = self.builder.theme().get_id(StyleClass::Label);
        let alignment = self
            .builder
            .theme()
            .get(StyleClass::Label)
            .text_align
            .get(self.state_flags);
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
            .upsert_dynamic_text_layout(self.builder.id);

        if is_focused {
            Self::handle_keyboard_events(&input, dynamic_layout, self.builder.text_context);
        }

        Self::handle_mouse_events(
            &input,
            placement,
            has_text_layout,
            self.interaction,
            self.state_flags,
            dynamic_layout,
            self.builder.text_context,
        );

        theme.apply_plain_editor_styles(style_id, self.state_flags, &mut dynamic_layout.editor);

        let layout = dynamic_layout.editor.layout(
            &mut self.builder.text_context.fonts,
            &mut self.builder.text_context.layouts,
        );

        let size = layout.calculate_content_widths();

        let (cursor_size, selection_color, cursor_color) = if is_focused {
            let style = theme.get(StyleClass::Label);
            let sel_color = style.selection_color.get(self.state_flags);
            let cur_color = style.cursor_color.get(self.state_flags);
            let cursor_sz = style.font_size.get(self.state_flags) as f32;

            (cursor_sz, sel_color, cur_color)
        } else {
            Default::default()
        };

        self.builder.context.ui_tree.add(
            Some(self.builder.index),
            Atom {
                width: Size::Flex {
                    min: size.min,
                    max: size.max,
                },
                height: Size::Fit {
                    min: 0.0,
                    max: f32::MAX,
                },
                ..Default::default()
            },
            (
                LayoutContent::Text {
                    layout: text_layout_id,
                    alignment,
                    cursor_size,
                    selection_color,
                    cursor_color,
                },
                None,
            ),
        );

        self.builder
            .set_active(self.state_flags.contains(StateFlags::PRESSED));
        self.builder
            .apply_style(StyleClass::Label, self.state_flags);

        let (_, dynamic_layout) = self
            .builder
            .context
            .upsert_dynamic_text_layout(self.builder.id);

        let is_composing = dynamic_layout.editor.is_composing();
        let text = (!is_composing).then_some(dynamic_layout.editor.raw_text());

        (text, self.interaction)
    }

    fn handle_keyboard_events(
        input: &crate::ui::Input,
        dynamic_layout: &mut crate::ui::text::DynamicTextLayout,
        text_context: &mut crate::graphics::TextLayoutContext,
    ) {
        let mut driver = dynamic_layout
            .editor
            .driver(&mut text_context.fonts, &mut text_context.layouts);

        let modifiers = input.modifiers;
        for event in input.keyboard_events.iter() {
            if !event.state.is_pressed() {
                continue;
            }

            // Check for modifier keys
            let is_ctrl = matches!(
                event.key,
                PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight)
            );
            let is_shift = matches!(
                event.key,
                PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight)
            );

            // Skip modifier keys themselves
            if is_ctrl || is_shift {
                continue;
            }

            // Use current modifier state for ctrl/shift
            let ctrl_held = modifiers.control_key();
            let shift_held = modifiers.shift_key();

            match event.key {
                PhysicalKey::Code(KeyCode::KeyA) if ctrl_held => {
                    driver.select_all();
                }
                PhysicalKey::Code(KeyCode::ArrowLeft) => match (ctrl_held, shift_held) {
                    (true, true) => driver.select_word_left(),
                    (true, false) => driver.move_word_left(),
                    (false, true) => driver.select_left(),
                    (false, false) => driver.move_left(),
                },
                PhysicalKey::Code(KeyCode::ArrowRight) => match (ctrl_held, shift_held) {
                    (true, true) => driver.select_word_right(),
                    (true, false) => driver.move_word_right(),
                    (false, true) => driver.select_right(),
                    (false, false) => driver.move_right(),
                },
                PhysicalKey::Code(KeyCode::ArrowUp) => {
                    if shift_held {
                        driver.select_up();
                    } else {
                        driver.move_up();
                    }
                }
                PhysicalKey::Code(KeyCode::ArrowDown) => {
                    if shift_held {
                        driver.select_down();
                    } else {
                        driver.move_down();
                    }
                }
                PhysicalKey::Code(KeyCode::PageUp) => {
                    if shift_held {
                        driver.select_up();
                    } else {
                        driver.move_up();
                    }
                }
                PhysicalKey::Code(KeyCode::PageDown) => {
                    if shift_held {
                        driver.select_down();
                    } else {
                        driver.move_down();
                    }
                }
                PhysicalKey::Code(KeyCode::Backspace) => {
                    if ctrl_held {
                        driver.backdelete_word();
                    } else {
                        driver.backdelete();
                    }
                }
                PhysicalKey::Code(KeyCode::Delete) => {
                    if ctrl_held {
                        driver.delete_word();
                    } else {
                        driver.delete();
                    }
                }
                PhysicalKey::Code(KeyCode::Home) => match (ctrl_held, shift_held) {
                    (true, true) => driver.select_to_text_start(),
                    (true, false) => driver.move_to_text_start(),
                    (false, true) => driver.select_to_line_start(),
                    (false, false) => driver.move_to_line_start(),
                },
                PhysicalKey::Code(KeyCode::End) => match (ctrl_held, shift_held) {
                    (true, true) => driver.select_to_text_end(),
                    (true, false) => driver.move_to_text_end(),
                    (false, true) => driver.select_to_line_end(),
                    (false, false) => driver.move_to_line_end(),
                },
                _ => {
                    if let Some(text) = &event.text {
                        driver.insert_or_replace_selection(text.as_str());
                    }
                }
            }
        }
    }

    fn handle_mouse_events(
        input: &crate::ui::Input,
        placement: glamour::Rect<crate::ui::Pixels>,
        has_text_layout: bool,
        interaction: Interaction,
        state_flags: StateFlags,
        dynamic_layout: &mut crate::ui::text::DynamicTextLayout,
        text_context: &mut crate::graphics::TextLayoutContext,
    ) {
        let left_click_count = input.mouse_state.left_click_count;
        if left_click_count == 0 {
            return;
        }

        let is_hovered = interaction.is_hovered;
        let is_focused = state_flags.contains(StateFlags::FOCUSED);
        if !is_hovered && !is_focused {
            return;
        }

        if !has_text_layout {
            return;
        }

        let pointer = input.pointer;
        let max_x = placement.origin.x + placement.size.width;
        let max_y = placement.origin.y + placement.size.height;
        let clamped_x = pointer.x.clamp(placement.origin.x, max_x);
        let clamped_y = pointer.y.clamp(placement.origin.y, max_y);

        let local_x = clamped_x - placement.origin.x;
        let local_y = clamped_y - placement.origin.y;
        let shift_held = input.modifiers.shift_key();
        let is_left_down = input.mouse_state.is_left_down();

        let mut driver = dynamic_layout
            .editor
            .driver(&mut text_context.fonts, &mut text_context.layouts);

        let layout = driver.layout();
        let cursor = Cursor::from_point(layout, local_x, local_y);
        let byte_index = cursor.index();

        if interaction.is_clicked && is_hovered {
            if left_click_count >= 4 {
                driver.select_all();
                return;
            }

            if left_click_count == 3 {
                driver.select_line_at_point(local_x, local_y);
                return;
            }

            if left_click_count == 2 {
                driver.select_word_at_point(local_x, local_y);
                return;
            }

            if shift_held {
                driver.extend_selection_to_byte(byte_index);
            } else {
                driver.move_to_byte(byte_index);
            }

            return;
        }

        if is_left_down && is_focused {
            if left_click_count == 3 {
                driver.select_line_at_point(local_x, local_y);
            } else if left_click_count == 2 {
                driver.select_word_at_point(local_x, local_y);
            } else {
                driver.extend_selection_to_byte(byte_index);
            }
        }
    }
}
