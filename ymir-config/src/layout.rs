use ymir_ipc::{ColumnDisplay, SizeChange};

use crate::appearance::{Border, FocusRing, InsertHint, Shadow, DEFAULT_BACKGROUND_COLOR};
use crate::utils::{Flag, MergeWith};
use crate::{BorderRule, Color, FloatOrInt, InsertHintPart, ShadowRule};

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub focus_ring: FocusRing,
    pub border: Border,
    pub shadow: Shadow,
    pub insert_hint: InsertHint,
    pub preset_column_widths: Vec<PresetSize>,
    pub default_column_width: Option<PresetSize>,
    pub preset_window_heights: Vec<PresetSize>,
    pub center_focused_column: CenterFocusedColumn,
    pub always_center_single_column: bool,
    pub empty_workspace_above_first: bool,
    pub default_column_display: ColumnDisplay,
    /// How many windows a dwindle column may hold before a fresh full-width dwindle column
    /// starts next to it on the strip (an additional dwindle "page"). A larger value grows a
    /// deeper split tree; a smaller one scrolls sideways sooner.
    pub dwindle_windows_per_column: usize,
    /// How the split side is chosen when a new window is opened in a dwindle column (Hyprland's
    /// `force_split`).
    pub dwindle_force_split: DwindleForceSplit,
    /// Prefer the newly opened window when splitting (Hyprland's `split_bias`).
    pub dwindle_split_bias: bool,
    /// Never re-evaluate the split orientation of a dwindle container once set (Hyprland's
    /// `preserve_split`).
    pub dwindle_preserve_split: bool,
    /// Multiplier used when deciding whether to split left/right vs top/bottom by aspect ratio
    /// (Hyprland's `split_width_multiplier`). Larger values favor top/bottom splits.
    pub dwindle_split_width_multiplier: f64,
    /// Default dwindle split resource: 1.0 is an even split (Hyprland's `default_split_ratio`,
    /// range `[0.1, 1.9]`; values above 1.0 grow the second window's share).
    pub dwindle_default_split_ratio: f64,
    /// Use cursor position to pick the split direction inside a dwindle column (Hyprland's
    /// `smart_split`).
    pub dwindle_smart_split: bool,
    /// Keep a preselected direction active for every subsequent window until reset (Hyprland's
    /// `permanent_direction_override`).
    pub dwindle_permanent_direction_override: bool,
    pub gaps: f64,
    pub struts: Struts,
    pub background_color: Color,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            focus_ring: FocusRing::default(),
            border: Border::default(),
            shadow: Shadow::default(),
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
            dwindle_windows_per_column: 8,
            dwindle_force_split: DwindleForceSplit::Second,
            dwindle_split_bias: false,
            dwindle_preserve_split: false,
            dwindle_split_width_multiplier: 1.,
            dwindle_default_split_ratio: 1.,
            dwindle_smart_split: false,
            dwindle_permanent_direction_override: false,
            gaps: 16.,
            struts: Struts::default(),
            preset_window_heights: vec![
                PresetSize::Proportion(1. / 3.),
                PresetSize::Proportion(0.5),
                PresetSize::Proportion(2. / 3.),
            ],
            background_color: DEFAULT_BACKGROUND_COLOR,
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
            insert_hint,
            always_center_single_column,
            empty_workspace_above_first,
            dwindle_split_bias,
            dwindle_preserve_split,
            dwindle_split_width_multiplier,
            dwindle_default_split_ratio,
            dwindle_smart_split,
            dwindle_permanent_direction_override,
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
            dwindle_force_split,
        );

        if let Some(x) = part.dwindle_windows_per_column {
            self.dwindle_windows_per_column = x;
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
    pub insert_hint: Option<InsertHintPart>,
    pub preset_column_widths: Option<Vec<PresetSize>>,
    pub default_column_width: Option<DefaultPresetSize>,
    pub preset_window_heights: Option<Vec<PresetSize>>,
    pub center_focused_column: Option<CenterFocusedColumn>,
    pub always_center_single_column: Option<Flag>,
    pub empty_workspace_above_first: Option<Flag>,
    pub default_column_display: Option<ColumnDisplay>,
    pub dwindle_windows_per_column: Option<usize>,
    pub dwindle_force_split: Option<DwindleForceSplit>,
    pub dwindle_split_bias: Option<Flag>,
    pub dwindle_preserve_split: Option<Flag>,
    pub dwindle_split_width_multiplier: Option<FloatOrInt<0, 65535>>,
    pub dwindle_default_split_ratio: Option<FloatOrInt<1, 19>>,
    pub dwindle_smart_split: Option<Flag>,
    pub dwindle_permanent_direction_override: Option<Flag>,
    pub gaps: Option<FloatOrInt<0, 65535>>,
    pub struts: Option<Struts>,
    pub background_color: Option<Color>,
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

/// How the side is chosen when a new window is opened in a dwindle column.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum DwindleForceSplit {
    /// Split according to the aspect ratio / cursor position of the focused window.
    #[default]
    Auto,
    /// The new window always takes the top or left half (the `First` child).
    First,
    /// The new window always takes the bottom or right half (the `Second` child).
    Second,
}
