use ymir_ipc::{ColumnDisplay, SizeChange};

use crate::animations::SpringParams;
use crate::appearance::{
    Border, FocusRing, InsertHint, Shadow, ShadowOffset, TabIndicator, DEFAULT_BACKGROUND_COLOR,
};
use crate::utils::{Flag, MergeWith};
use crate::{BorderRule, Color, FloatOrInt, InsertHintPart, ShadowRule, TabIndicatorPart};

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub focus_ring: FocusRing,
    pub border: Border,
    pub shadow: Shadow,
    pub tab_indicator: TabIndicator,
    pub insert_hint: InsertHint,
    pub preset_column_widths: Vec<PresetSize>,
    pub default_column_width: Option<PresetSize>,
    pub preset_window_heights: Vec<PresetSize>,
    pub center_focused_column: CenterFocusedColumn,
    pub always_center_single_column: bool,
    pub empty_workspace_above_first: bool,
    pub default_column_display: ColumnDisplay,
    pub gaps: f64,
    pub struts: Struts,
    pub background_color: Color,
    pub depth_queue: DepthQueue,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            focus_ring: FocusRing::default(),
            border: Border::default(),
            shadow: Shadow::default(),
            tab_indicator: TabIndicator::default(),
            insert_hint: InsertHint::default(),
            preset_column_widths: vec![
                PresetSize::Proportion(1. / 3.),
                PresetSize::Proportion(0.5),
                PresetSize::Proportion(2. / 3.),
            ],
            default_column_width: Some(PresetSize::Proportion(0.5)),
            center_focused_column: CenterFocusedColumn::Never,
            always_center_single_column: false,
            empty_workspace_above_first: false,
            default_column_display: ColumnDisplay::Normal,
            gaps: 16.,
            struts: Struts::default(),
            preset_window_heights: vec![
                PresetSize::Proportion(1. / 3.),
                PresetSize::Proportion(0.5),
                PresetSize::Proportion(2. / 3.),
            ],
            background_color: DEFAULT_BACKGROUND_COLOR,
            depth_queue: DepthQueue::default(),
        }
    }
}

impl MergeWith<LayoutPart> for Layout {
    fn merge_with(&mut self, part: &LayoutPart) {
        merge!(
            (self, part),
            focus_ring,
            border,
            shadow,
            tab_indicator,
            insert_hint,
            always_center_single_column,
            empty_workspace_above_first,
            gaps,
        );

        merge_clone!(
            (self, part),
            preset_column_widths,
            preset_window_heights,
            center_focused_column,
            default_column_display,
            struts,
            background_color,
        );

        if let Some(x) = &part.depth_queue {
            self.depth_queue.merge_with(x);
        }

        if let Some(x) = part.default_column_width {
            self.default_column_width = x.0;
        }

        if self.preset_column_widths.is_empty() {
            self.preset_column_widths = Layout::default().preset_column_widths;
        }

        if self.preset_window_heights.is_empty() {
            self.preset_window_heights = Layout::default().preset_window_heights;
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct LayoutPart {
    pub focus_ring: Option<BorderRule>,
    pub border: Option<BorderRule>,
    pub shadow: Option<ShadowRule>,
    pub tab_indicator: Option<TabIndicatorPart>,
    pub insert_hint: Option<InsertHintPart>,
    pub preset_column_widths: Option<Vec<PresetSize>>,
    pub default_column_width: Option<DefaultPresetSize>,
    pub preset_window_heights: Option<Vec<PresetSize>>,
    pub center_focused_column: Option<CenterFocusedColumn>,
    pub always_center_single_column: Option<Flag>,
    pub empty_workspace_above_first: Option<Flag>,
    pub default_column_display: Option<ColumnDisplay>,
    pub gaps: Option<FloatOrInt<0, 65535>>,
    pub struts: Option<Struts>,
    pub background_color: Option<Color>,
    pub depth_queue: Option<DepthQueuePart>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PresetSize {
    Proportion(f64),
    Fixed(i32),
}

impl From<PresetSize> for SizeChange {
    fn from(value: PresetSize) -> Self {
        match value {
            PresetSize::Proportion(prop) => SizeChange::SetProportion(prop * 100.),
            PresetSize::Fixed(fixed) => SizeChange::SetFixed(fixed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefaultPresetSize(pub Option<PresetSize>);

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Struts {
    pub left: FloatOrInt<-65535, 65535>,
    pub right: FloatOrInt<-65535, 65535>,
    pub top: FloatOrInt<-65535, 65535>,
    pub bottom: FloatOrInt<-65535, 65535>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum CenterFocusedColumn {
    /// Focusing a column will not center the column.
    #[default]
    Never,
    /// The focused column will always be centered.
    Always,
    /// Focusing a column will center it if it doesn't fit on the screen together with the
    /// previously focused column.
    OnOverflow,
}

/// Depth-queue ("depth") column display mode settings.
#[derive(Debug, Clone, PartialEq)]
pub struct DepthQueue {
    /// Height of the apex card relative to the working area height (0..1).
    pub card_height_ratio: f64,
    /// Number of cards visible in the top deck fan (above the apex).
    pub top_deck_size: usize,
    /// Number of cards visible in the bottom deck fan (below the apex).
    pub bottom_deck_size: usize,
    /// Vertical gap between consecutive cards in the decks.
    pub gap: f64,
    /// How far a deck card bleeds past the working area edge.
    pub deck_bleed: f64,
    /// Opacity of the farthest (most occluded) card in a deck; the apex is always fully opaque.
    pub min_opacity: f64,
    /// Blur radius applied to the backdrop behind the decks (0 disables the blur).
    pub blur_radius: f64,
    /// Shadow cast by each deck card.
    pub card_shadow: DepthDeckShadow,
    /// Perspective tilt of the deck fans, in degrees (0 disables the tilt).
    pub perspective_tilt: f64,
    /// Spring that drives the focus shuffle between cards.
    pub focus_shuffle: SpringParams,
}

impl Default for DepthQueue {
    fn default() -> Self {
        Self {
            card_height_ratio: 0.62,
            top_deck_size: 2,
            bottom_deck_size: 2,
            gap: 12.,
            deck_bleed: 24.,
            min_opacity: 0.35,
            blur_radius: 18.,
            card_shadow: DepthDeckShadow::default(),
            perspective_tilt: 7.,
            focus_shuffle: SpringParams {
                damping_ratio: 0.62,
                stiffness: 750,
                epsilon: 0.0001,
            },
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DepthQueuePart {
    pub card_height_ratio: Option<FloatOrInt<0, 10000>>,
    pub top_deck_size: Option<usize>,
    pub bottom_deck_size: Option<usize>,
    pub gap: Option<FloatOrInt<0, 65535>>,
    pub deck_bleed: Option<FloatOrInt<0, 65535>>,
    pub min_opacity: Option<FloatOrInt<0, 10000>>,
    pub blur_radius: Option<FloatOrInt<0, 65535>>,
    pub card_shadow: Option<DepthDeckShadowPart>,
    pub perspective_tilt: Option<FloatOrInt<0, 360>>,
    pub focus_shuffle: Option<SpringPart>,
}

impl MergeWith<DepthQueuePart> for DepthQueue {
    fn merge_with(&mut self, part: &DepthQueuePart) {
        if let Some(x) = part.card_height_ratio {
            self.card_height_ratio = x.0;
        }
        if let Some(x) = part.top_deck_size {
            self.top_deck_size = x;
        }
        if let Some(x) = part.bottom_deck_size {
            self.bottom_deck_size = x;
        }
        if let Some(x) = part.gap {
            self.gap = x.0;
        }
        if let Some(x) = part.deck_bleed {
            self.deck_bleed = x.0;
        }
        if let Some(x) = part.min_opacity {
            self.min_opacity = x.0;
        }
        if let Some(x) = part.blur_radius {
            self.blur_radius = x.0;
        }
        if let Some(x) = &part.card_shadow {
            self.card_shadow.merge_with(x);
        }
        if let Some(x) = part.perspective_tilt {
            self.perspective_tilt = x.0;
        }
        if let Some(x) = part.focus_shuffle.as_ref() {
            if let Some(x) = x.damping_ratio {
                self.focus_shuffle.damping_ratio = x.0;
            }
            if let Some(x) = x.stiffness {
                self.focus_shuffle.stiffness = x;
            }
            if let Some(x) = x.epsilon {
                self.focus_shuffle.epsilon = x.0;
            }
        }
    }
}

/// Shadow settings for depth deck cards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthDeckShadow {
    pub on: bool,
    pub offset: ShadowOffset,
    pub blur: f64,
    pub color: Color,
}

impl Default for DepthDeckShadow {
    fn default() -> Self {
        Self {
            on: true,
            offset: ShadowOffset {
                x: FloatOrInt(0.),
                y: FloatOrInt(10.),
            },
            blur: 24.,
            color: Color::from_rgba8_unpremul(0, 0, 0, 0x45),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DepthDeckShadowPart {
    pub on: bool,
    pub offset: Option<ShadowOffset>,
    pub blur: Option<FloatOrInt<0, 1024>>,
    pub color: Option<Color>,
}

impl MergeWith<DepthDeckShadowPart> for DepthDeckShadow {
    fn merge_with(&mut self, part: &DepthDeckShadowPart) {
        self.on |= part.on;
        if let Some(x) = part.offset {
            self.offset = x;
        }
        if let Some(x) = part.blur {
            self.blur = x.0;
        }
        if let Some(x) = part.color {
            self.color = x;
        }
    }
}

/// Spring parameters ("focus_shuffle") for the depth queue shuffle.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct SpringPart {
    pub damping_ratio: Option<FloatOrInt<0, 100000>>,
    pub stiffness: Option<u32>,
    pub epsilon: Option<FloatOrInt<0, 100000>>,
}

