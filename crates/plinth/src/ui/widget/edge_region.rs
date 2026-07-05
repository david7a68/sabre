use crate::ui::UiBuilder;

use super::SplitPaneConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug)]
pub struct EdgeRegionState {
    pub fraction: f32,
    pub is_resizing: bool,
}

impl EdgeRegionState {
    pub fn new(fraction: f32) -> Self {
        Self {
            fraction,
            is_resizing: false,
        }
    }

    pub fn show(
        &mut self,
        builder: &mut UiBuilder<'_>,
        config: EdgeRegionConfig,
        region: impl FnOnce(&mut UiBuilder<'_>),
        remaining: impl FnOnce(&mut UiBuilder<'_>),
    ) {
        let pane_config = match config.edge {
            Edge::Left | Edge::Right => SplitPaneConfig::horizontal(),
            Edge::Top | Edge::Bottom => SplitPaneConfig::vertical(),
        }
        .with_min_first(match config.edge {
            Edge::Left | Edge::Top => config.min_region,
            Edge::Right | Edge::Bottom => config.min_remaining,
        })
        .with_min_second(match config.edge {
            Edge::Left | Edge::Top => config.min_remaining,
            Edge::Right | Edge::Bottom => config.min_region,
        })
        .with_resizable(config.resizable)
        .with_divider_thickness(config.divider_thickness);

        match config.edge {
            Edge::Left | Edge::Top => super::split_pane::render_split_pane_parts(
                builder,
                &mut self.fraction,
                &mut self.is_resizing,
                pane_config,
                region,
                remaining,
            ),
            Edge::Right | Edge::Bottom => super::split_pane::render_split_pane_parts(
                builder,
                &mut self.fraction,
                &mut self.is_resizing,
                pane_config,
                remaining,
                region,
            ),
        }
    }
}

impl Default for EdgeRegionState {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EdgeRegionConfig {
    pub edge: Edge,
    pub min_region: f32,
    pub min_remaining: f32,
    pub resizable: bool,
    pub divider_thickness: f32,
}

impl EdgeRegionConfig {
    pub fn left() -> Self {
        Self {
            edge: Edge::Left,
            ..Self::default()
        }
    }

    pub fn right() -> Self {
        Self {
            edge: Edge::Right,
            ..Self::default()
        }
    }

    pub fn top() -> Self {
        Self {
            edge: Edge::Top,
            ..Self::default()
        }
    }

    pub fn bottom() -> Self {
        Self {
            edge: Edge::Bottom,
            ..Self::default()
        }
    }

    pub fn with_min_region(mut self, min_region: f32) -> Self {
        self.min_region = min_region;
        self
    }

    pub fn with_min_remaining(mut self, min_remaining: f32) -> Self {
        self.min_remaining = min_remaining;
        self
    }

    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
}

impl Default for EdgeRegionConfig {
    fn default() -> Self {
        Self {
            edge: Edge::Left,
            min_region: 80.0,
            min_remaining: 80.0,
            resizable: true,
            divider_thickness: 6.0,
        }
    }
}
