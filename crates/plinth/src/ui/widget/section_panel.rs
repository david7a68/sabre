use crate::graphics::Color;
use crate::ui::Padding;
use crate::ui::Size;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

use super::ClickBehavior;
use super::Interaction;

#[derive(Clone, Copy, Debug)]
pub struct SectionPanelState {
    pub is_open: bool,
}

impl SectionPanelState {
    pub fn open() -> Self {
        Self { is_open: true }
    }

    pub fn closed() -> Self {
        Self { is_open: false }
    }

    pub fn show(
        &mut self,
        builder: &mut UiBuilder<'_>,
        title: &str,
        config: SectionPanelConfig,
        body: impl FnOnce(&mut UiBuilder<'_>),
    ) {
        builder.width(Size::Grow);

        let mut header = builder.named_child((title, "header"));
        let (interaction, flags) = Interaction::compute(
            &header,
            ClickBehavior::OnPress,
            StateFlags::HOVERED | StateFlags::PRESSED,
        );
        if interaction.is_activated {
            self.is_open = !self.is_open;
        }
        header
            .width(Size::Grow)
            .height(config.header_height)
            .padding(Padding {
                left: 8.0,
                right: 8.0,
                top: 4.0,
                bottom: 4.0,
            })
            .color(
                if flags.intersects(StateFlags::HOVERED | StateFlags::PRESSED) {
                    config.header_hover_color
                } else {
                    config.header_color
                },
            );
        header.text(if self.is_open { "▾ " } else { "▸ " }, None);
        header.text(title, None);
        header.set_active(flags.contains(StateFlags::PRESSED));
        drop(header);

        if self.is_open {
            builder.with_named_child((title, "body"), |ui| {
                ui.width(Size::Grow)
                    .padding(Padding {
                        left: 10.0,
                        right: 10.0,
                        top: 8.0,
                        bottom: 8.0,
                    })
                    .color(config.body_color);
                body(ui);
            });
        }
    }
}

impl Default for SectionPanelState {
    fn default() -> Self {
        Self::open()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SectionPanelConfig {
    pub header_height: f32,
    pub header_color: Color,
    pub header_hover_color: Color,
    pub body_color: Color,
}

impl Default for SectionPanelConfig {
    fn default() -> Self {
        Self {
            header_height: 34.0,
            header_color: Color::srgb_nonlinear(0.16, 0.16, 0.18, 1.0),
            header_hover_color: Color::srgb_nonlinear(0.24, 0.24, 0.27, 1.0),
            body_color: Color::srgb_nonlinear(0.11, 0.11, 0.13, 1.0),
        }
    }
}
