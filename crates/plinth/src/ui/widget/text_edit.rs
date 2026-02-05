use parley::Cursor;
use smallvec::SmallVec;
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
    is_active: bool,
    is_focused: bool,
    state_flags: StateFlags,
    wrap_text: bool,
}

impl<'a> TextEdit<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>, initial_text: &str, width: Size) -> Self {
        let mut builder = builder.child();
        builder.width(width);

        let prev_state = builder.prev_state();
        let input = builder.input();
        let is_focused_prev = builder.is_focused();

        let (interaction, is_active) =
            Interaction::compute(prev_state, input, ClickBehavior::OnPress);

        let is_double_click = interaction.is_hovered && input.mouse_state.left_click_count == 2;

        // Focus management: gain focus on click, keep focus if already focused
        let is_focused = if interaction.is_clicked {
            builder.request_focus();
            true
        } else if is_focused_prev && interaction.is_hovered && is_active {
            // Clicking in bounds while focused - stay focused
            true
        } else if is_focused_prev && !interaction.is_hovered && input.mouse_state.is_left_down() {
            // Clicking outside while focused - lose focus
            builder.release_focus();
            false
        } else {
            is_focused_prev
        };

        let (_, dynamic_layout) = builder.context.upsert_dynamic_text_layout(builder.id);

        let has_text = dynamic_layout.editor.raw_text().is_empty();
        if !has_text && !initial_text.is_empty() {
            dynamic_layout.editor.set_text(initial_text);
        }

        let mut driver = dynamic_layout.editor.driver(
            &mut builder.text_context.fonts,
            &mut builder.text_context.layouts,
        );

        // Handle double-click select all
        if is_double_click && is_focused {
            driver.select_all();
        }

        let mut state_flags = StateFlags::NORMAL;
        if interaction.is_hovered {
            state_flags |= StateFlags::HOVERED;
        }
        if is_active {
            state_flags |= StateFlags::PRESSED;
        }
        if is_focused {
            state_flags |= StateFlags::FOCUSED;
        }

        Self {
            builder,
            interaction,
            is_active,
            is_focused,
            state_flags,
            wrap_text: false,
        }
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap_text = wrap;
        self
    }

    pub fn finish(mut self) -> (Option<&'a str>, Interaction) {
        // Process keyboard events if focused
        if self.is_focused {
            self.handle_keyboard_events();
        }

        // Handle mouse events for cursor positioning and selection
        if self.interaction.is_clicked || (self.is_active && self.is_focused) {
            self.handle_mouse_events();
        }

        // Apply styling and render
        self.render();

        // Get text from editor
        let (_, dynamic_layout) = self
            .builder
            .context
            .upsert_dynamic_text_layout(self.builder.id);

        // Check if IME composition is in progress
        let text = if dynamic_layout.editor.is_composing() {
            None
        } else {
            // Store text in context's buffer and return reference
            // let text_string = dynamic_layout.editor.raw_text().to_string();
            // self.builder.context.text_buffers.push(text_string);
            // Some(self.builder.context.text_buffers.last().unwrap().as_str())
            Some(dynamic_layout.editor.raw_text())
        };

        (text, self.interaction)
    }

    fn handle_keyboard_events(&mut self) {
        let keyboard_events = self.builder.input().keyboard_events.clone();
        let modifiers = self.builder.input().modifiers;
        let (_, dynamic_layout) = self
            .builder
            .context
            .upsert_dynamic_text_layout(self.builder.id);

        let mut driver = dynamic_layout.editor.driver(
            &mut self.builder.text_context.fonts,
            &mut self.builder.text_context.layouts,
        );

        for event in keyboard_events.iter() {
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
                PhysicalKey::Code(KeyCode::ArrowLeft) => {
                    if ctrl_held {
                        if shift_held {
                            driver.select_word_left();
                        } else {
                            driver.move_word_left();
                        }
                    } else if shift_held {
                        driver.select_left();
                    } else {
                        driver.move_left();
                    }
                }
                PhysicalKey::Code(KeyCode::ArrowRight) => {
                    if ctrl_held {
                        if shift_held {
                            driver.select_word_right();
                        } else {
                            driver.move_word_right();
                        }
                    } else if shift_held {
                        driver.select_right();
                    } else {
                        driver.move_right();
                    }
                }
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
                PhysicalKey::Code(KeyCode::Home) => {
                    if ctrl_held {
                        if shift_held {
                            driver.select_to_text_start();
                        } else {
                            driver.move_to_text_start();
                        }
                    } else if shift_held {
                        driver.select_to_line_start();
                    } else {
                        driver.move_to_line_start();
                    }
                }
                PhysicalKey::Code(KeyCode::End) => {
                    if ctrl_held {
                        if shift_held {
                            driver.select_to_text_end();
                        } else {
                            driver.move_to_text_end();
                        }
                    } else if shift_held {
                        driver.select_to_line_end();
                    } else {
                        driver.move_to_line_end();
                    }
                }
                _ => {
                    // Handle character input
                    if let Some(text) = &event.text {
                        // Filter out control characters
                        if !text.chars().any(|c| c.is_control()) {
                            driver.insert_or_replace_selection(text.as_str());
                        }
                    }
                }
            }
        }
    }

    fn handle_mouse_events(&mut self) {
        use glamour::Contains;

        // Get needed values before any mutable borrows
        let pointer = self.builder.input().pointer;
        let is_left_down = self.builder.input().mouse_state.is_left_down();
        let placement = self
            .builder
            .prev_state()
            .map(|s| s.placement)
            .unwrap_or_default();
        let has_text_layout = self
            .builder
            .prev_state()
            .and_then(|s| s.text_layout)
            .is_some();
        let is_clicked = self.interaction.is_clicked;
        let is_active = self.is_active;

        // Check if we're in a click or drag state
        let mouse_in_bounds = placement.contains(&pointer);

        if !mouse_in_bounds || !is_left_down {
            return;
        }

        // Convert mouse position to widget-relative coordinates
        let local_x = pointer.x - placement.origin.x;
        let local_y = pointer.y - placement.origin.y;

        // Get the editor and use hit testing to find cursor position
        let (_, dynamic_layout) = self
            .builder
            .context
            .upsert_dynamic_text_layout(self.builder.id);
        let mut driver = dynamic_layout.editor.driver(
            &mut self.builder.text_context.fonts,
            &mut self.builder.text_context.layouts,
        );

        // Use parley's Cursor::from_point to hit test
        let layout = driver.layout();
        let cursor = Cursor::from_point(layout, local_x, local_y);
        let byte_index = cursor.index();

        if is_active && has_text_layout {
            // If we're actively dragging (was already active), extend selection
            driver.extend_selection_to_byte(byte_index);
        } else if is_clicked {
            // Fresh click - move cursor to position
            driver.move_to_byte(byte_index);
        }
    }

    fn render(&mut self) {
        let style_id = self.builder.theme().get_id(StyleClass::Label);
        let state_flags_val = self.state_flags;

        // Get alignment before mutable borrows
        let alignment = self
            .builder
            .theme()
            .get(StyleClass::Label)
            .text_align
            .get(state_flags_val);

        // Get theme reference before mutable borrow of context
        let theme = self.builder.theme;

        let (text_layout_id, dynamic_layout) = self
            .builder
            .context
            .upsert_dynamic_text_layout(self.builder.id);

        // Always apply styles before layout to ensure font features like kerning are applied
        theme.apply_plain_editor_styles(style_id, state_flags_val, &mut dynamic_layout.editor);

        dynamic_layout.style_id = style_id;
        dynamic_layout.state_flags = self.state_flags;
        dynamic_layout.styles_dirty = false;

        // Set wrapping - width will be set during layout phase
        // For now, None means single-line mode
        dynamic_layout.editor.set_width(None);

        // Get layout and size
        let layout = dynamic_layout.editor.layout(
            &mut self.builder.text_context.fonts,
            &mut self.builder.text_context.layouts,
        );
        let size = layout.calculate_content_widths();

        dynamic_layout.layout = layout.clone();

        let (selection_rects, cursor_rect, selection_color, cursor_color) = if self.is_focused {
            let style = theme.get(StyleClass::Label);
            let sel_color = style.selection_color.get(state_flags_val);
            let cur_color = style.cursor_color.get(state_flags_val);
            let cursor_size = style.font_size.get(state_flags_val) as f32;

            let mut sel_rects: SmallVec<[parley::BoundingBox; 8]> = SmallVec::new();
            dynamic_layout
                .editor
                .selection_geometry_with(|bbox, _| sel_rects.push(bbox));

            let sel_rects = match dynamic_layout.editor.selected_text() {
                Some(selected) if !selected.is_empty() && !sel_rects.is_empty() => Some(sel_rects),
                _ => None,
            };

            let cur_rect = dynamic_layout.editor.cursor_geometry(cursor_size);

            (sel_rects, cur_rect, Some(sel_color), Some(cur_color))
        } else {
            (None, None, None, None)
        };

        self.builder.context.ui_tree.add(
            Some(self.builder.index),
            Atom {
                width: Size::Flex {
                    min: size.min,
                    max: size.max,
                },
                height: Size::Fit {
                    min: 20.0,
                    max: f32::MAX,
                },
                ..Default::default()
            },
            (
                LayoutContent::Text {
                    layout: text_layout_id,
                    alignment,
                    selection_rects,
                    cursor_rect,
                    selection_color,
                    cursor_color,
                },
                None,
            ),
        );

        self.builder.set_active(self.is_active);
        self.builder.apply_style(StyleClass::Label, state_flags_val);
    }
}
