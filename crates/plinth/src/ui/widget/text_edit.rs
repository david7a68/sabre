use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

use crate::graphics::Color;
use crate::graphics::GradientPaint;
use crate::graphics::Paint;
use crate::shell::Clipboard;
use crate::shell::Input;
use crate::ui::Size;
use crate::ui::builder::UiBuilder;
use crate::ui::style::BorderWidths;
use crate::ui::style::CornerRadii;
use crate::ui::style::StateFlags;
use crate::ui::text::EditableText;
use crate::ui::text::EditableTextContent;
use crate::ui::text::EditableTextHandle;
use crate::ui::text::TextEditCommand;
use crate::ui::text::TextServices;
use crate::ui::theme::StyleClass;
use crate::ui::widget::ClickBehavior;
use crate::ui::widget::Interaction;

use super::macros::forward_properties;

pub struct TextEdit<'a> {
    builder: UiBuilder<'a>,
    text: EditableTextHandle,
    interaction: Interaction,
    state_flags: StateFlags,
}

impl<'a> TextEdit<'a> {
    pub fn new(builder: &'a mut UiBuilder<'_>, text: &mut impl EditableText, width: Size) -> Self {
        let mut builder = builder.child();
        let text = text.handle();

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
            text,
            interaction,
            state_flags,
        }
    }

    forward_properties!(width, height);

    pub fn set_text(&mut self, text: &str) -> &mut Self {
        self.text.borrow_mut().set_text(text);
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
        let is_focused = self.state_flags.contains(StateFlags::FOCUSED);

        if is_focused {
            self.builder.request_focus();
        } else {
            self.builder.release_focus();
        }

        let style_id = self.builder.theme().get_id(StyleClass::TextEdit);
        let theme = self.builder.theme;

        let input = self.builder.input().clone();
        let placement = self.builder.prev_state().map(|s| s.placement);
        let services = self.builder.text_services.clone();

        let mut paint = self
            .text
            .borrow_mut()
            .apply_style(theme, style_id, self.state_flags);

        if let Some(placement) = placement {
            Self::handle_mouse_events(
                &self.text,
                &services,
                &input,
                placement,
                self.interaction,
                self.state_flags,
            );
        }

        if is_focused {
            Self::handle_keyboard_events(self.builder.clipboard, &services, &self.text, &input);
        }

        if !is_focused {
            paint.selection_color = Color::TRANSPARENT;
            paint.cursor_color = Color::TRANSPARENT;
        }

        let content = EditableTextContent::new(services, self.text.clone(), paint);
        self.builder.custom_content(
            Size::Grow,
            Size::Fit {
                min: paint.cursor_size,
                max: f32::MAX,
            },
            true,
            content,
        );

        self.interaction
    }

    fn handle_keyboard_events(
        clipboard: &mut Clipboard,
        services: &TextServices,
        text: &EditableTextHandle,
        input: &Input,
    ) {
        macro_rules! command {
            ($command:expr) => {
                text.borrow_mut().command(services, $command)
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
                    command!(TextEditCommand::SelectAll);
                }
                PhysicalKey::Code(KeyCode::ArrowLeft) => match (ctrl_held, shift_held) {
                    (true, true) => command!(TextEditCommand::MoveLeft {
                        select: true,
                        word: true,
                    }),
                    (true, false) => command!(TextEditCommand::MoveLeft {
                        select: false,
                        word: true,
                    }),
                    (false, true) => command!(TextEditCommand::MoveLeft {
                        select: true,
                        word: false,
                    }),
                    (false, false) => command!(TextEditCommand::MoveLeft {
                        select: false,
                        word: false,
                    }),
                },
                PhysicalKey::Code(KeyCode::ArrowRight) => match (ctrl_held, shift_held) {
                    (true, true) => command!(TextEditCommand::MoveRight {
                        select: true,
                        word: true,
                    }),
                    (true, false) => command!(TextEditCommand::MoveRight {
                        select: false,
                        word: true,
                    }),
                    (false, true) => command!(TextEditCommand::MoveRight {
                        select: true,
                        word: false,
                    }),
                    (false, false) => command!(TextEditCommand::MoveRight {
                        select: false,
                        word: false,
                    }),
                },
                PhysicalKey::Code(KeyCode::ArrowUp) => match shift_held {
                    true => command!(TextEditCommand::MoveUp { select: true }),
                    false => command!(TextEditCommand::MoveUp { select: false }),
                },
                PhysicalKey::Code(KeyCode::ArrowDown) => match shift_held {
                    true => command!(TextEditCommand::MoveDown { select: true }),
                    false => command!(TextEditCommand::MoveDown { select: false }),
                },
                PhysicalKey::Code(KeyCode::PageUp) => match shift_held {
                    true => command!(TextEditCommand::MoveUp { select: true }),
                    false => command!(TextEditCommand::MoveUp { select: false }),
                },
                PhysicalKey::Code(KeyCode::PageDown) => match shift_held {
                    true => command!(TextEditCommand::MoveDown { select: true }),
                    false => command!(TextEditCommand::MoveDown { select: false }),
                },
                PhysicalKey::Code(KeyCode::Backspace) => match ctrl_held {
                    true => command!(TextEditCommand::Backdelete { word: true }),
                    false => command!(TextEditCommand::Backdelete { word: false }),
                },
                PhysicalKey::Code(KeyCode::Delete) => match ctrl_held {
                    true => command!(TextEditCommand::Delete { word: true }),
                    false => command!(TextEditCommand::Delete { word: false }),
                },
                PhysicalKey::Code(KeyCode::Home) => match (ctrl_held, shift_held) {
                    (true, true) => command!(TextEditCommand::MoveToTextStart { select: true }),
                    (true, false) => command!(TextEditCommand::MoveToTextStart { select: false }),
                    (false, true) => command!(TextEditCommand::MoveToLineStart { select: true }),
                    (false, false) => command!(TextEditCommand::MoveToLineStart { select: false }),
                },
                PhysicalKey::Code(KeyCode::End) => match (ctrl_held, shift_held) {
                    (true, true) => command!(TextEditCommand::MoveToTextEnd { select: true }),
                    (true, false) => command!(TextEditCommand::MoveToTextEnd { select: false }),
                    (false, true) => command!(TextEditCommand::MoveToLineEnd { select: true }),
                    (false, false) => command!(TextEditCommand::MoveToLineEnd { select: false }),
                },
                PhysicalKey::Code(KeyCode::KeyC) if ctrl_held => {
                    let text = text.borrow();
                    if let Some(selected) = text.selected_text() {
                        clipboard.set_text(selected);
                    }
                }
                PhysicalKey::Code(KeyCode::KeyV) if ctrl_held => {
                    if let Some(text) = clipboard.get_text() {
                        command!(TextEditCommand::InsertOrReplaceSelection(&text));
                    }
                }
                _ => {
                    if let Some(text) = &event.text {
                        command!(TextEditCommand::InsertOrReplaceSelection(text.as_str()));
                    }
                }
            }
        }
    }

    fn handle_mouse_events(
        text: &EditableTextHandle,
        services: &TextServices,
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

        let command = if interaction.is_activated && is_hovered {
            match left_click_count {
                4.. => Some(TextEditCommand::SelectAll),
                3 => Some(TextEditCommand::SelectLineAtPoint {
                    x: local.x,
                    y: local.y,
                }),
                2 => Some(TextEditCommand::SelectWordAtPoint {
                    x: local.x,
                    y: local.y,
                }),
                1 if is_shift_held => Some(TextEditCommand::ExtendSelectionToPoint {
                    x: local.x,
                    y: local.y,
                }),
                1 => Some(TextEditCommand::MoveToPoint {
                    x: local.x,
                    y: local.y,
                }),
                0 => None,
            }
        } else if is_left_down && is_focused {
            match left_click_count {
                3 => Some(TextEditCommand::SelectLineAtPoint {
                    x: local.x,
                    y: local.y,
                }),
                2 => Some(TextEditCommand::SelectWordAtPoint {
                    x: local.x,
                    y: local.y,
                }),
                _ => Some(TextEditCommand::ExtendSelectionToPoint {
                    x: local.x,
                    y: local.y,
                }),
            }
        } else {
            None
        };

        if let Some(command) = command {
            text.borrow_mut().command(services, command);
        }
    }
}
