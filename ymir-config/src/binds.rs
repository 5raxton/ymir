use std::str::FromStr;
use std::time::Duration;

use bitflags::bitflags;
use miette::miette;
use ymir_ipc::{
    ColumnDisplay, LayoutSwitchTarget, PositionChange, SizeChange, SplitDirection,
    WorkspaceReferenceArg,
};
use smithay::input::keyboard::keysyms::KEY_NoSymbol;
use smithay::input::keyboard::xkb::{keysym_from_name, KEYSYM_CASE_INSENSITIVE, KEYSYM_NO_FLAGS};
use smithay::input::keyboard::Keysym;

use crate::recent_windows::{MruDirection, MruFilter, MruScope};
use crate::utils::MergeWith;

/// Direction of a dwindle split (used for the `preselect` action).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Top,
    Bottom,
    Left,
    Right,
}

impl FromStr for Direction {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "top" => Ok(Self::Top),
            "bottom" => Ok(Self::Bottom),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            _ => Err(r#"invalid direction, can be "top", "bottom", "left" or "right""#),
        }
    }
}

impl From<SplitDirection> for Direction {
    fn from(dir: SplitDirection) -> Self {
        match dir {
            SplitDirection::Top => Self::Top,
            SplitDirection::Bottom => Self::Bottom,
            SplitDirection::Left => Self::Left,
            SplitDirection::Right => Self::Right,
        }
    }
}

impl From<Direction> for SplitDirection {
    fn from(dir: Direction) -> Self {
        match dir {
            Direction::Top => Self::Top,
            Direction::Bottom => Self::Bottom,
            Direction::Left => Self::Left,
            Direction::Right => Self::Right,
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Binds(pub Vec<Bind>);

#[derive(Debug, Clone, PartialEq)]
pub struct Bind {
    pub key: Key,
    pub action: Action,
    pub repeat: bool,
    pub cooldown: Option<Duration>,
    pub allow_when_locked: bool,
    pub allow_inhibiting: bool,
    pub hotkey_overlay_title: Option<Option<String>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Key {
    pub trigger: Trigger,
    pub modifiers: Modifiers,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Trigger {
    Keysym(Keysym),
    MouseLeft,
    MouseRight,
    MouseMiddle,
    MouseBack,
    MouseForward,
    WheelScrollDown,
    WheelScrollUp,
    WheelScrollLeft,
    WheelScrollRight,
    TouchpadScrollDown,
    TouchpadScrollUp,
    TouchpadScrollLeft,
    TouchpadScrollRight,
    TabletStylusButton1,
    TabletStylusButton2,
    TabletStylusButton3,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers : u8 {
        const CTRL = 1;
        const SHIFT = 1 << 1;
        const ALT = 1 << 2;
        const SUPER = 1 << 3;
        const ISO_LEVEL3_SHIFT = 1 << 4;
        const ISO_LEVEL5_SHIFT = 1 << 5;
        const COMPOSITOR = 1 << 6;
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SwitchBinds {
    pub lid_open: Option<SwitchAction>,
    pub lid_close: Option<SwitchAction>,
    pub tablet_mode_on: Option<SwitchAction>,
    pub tablet_mode_off: Option<SwitchAction>,
}

impl MergeWith<SwitchBinds> for SwitchBinds {
    fn merge_with(&mut self, part: &SwitchBinds) {
        merge_clone_opt!(
            (self, part),
            lid_open,
            lid_close,
            tablet_mode_on,
            tablet_mode_off,
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchAction {
    pub spawn: Vec<String>,
}

// Remember to add new actions to the CLI enum too.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Quit(bool),
    ChangeVt(i32),
    Suspend,
    PowerOffMonitors,
    PowerOnMonitors,
    ToggleDebugTint,
    DebugToggleOpaqueRegions,
    DebugToggleDamage,
    Spawn(Vec<String>),
    SpawnSh(String),
    DoScreenTransition(Option<u16>),
    ConfirmScreenshot {
        write_to_disk: bool,
    },
    CancelScreenshot,
    ScreenshotTogglePointer,
    Screenshot(
        // Path; not settable from config
        Option<String>,
    ),
    ScreenshotScreen(
        // Path; not settable from config
        Option<String>,
    ),
    ScreenshotWindow(
        // Path; not settable from config
        Option<String>,
    ),
    ScreenshotWindowById {
        id: u64,
        write_to_disk: bool,
        show_pointer: bool,
        path: Option<String>,
    },
    ToggleKeyboardShortcutsInhibit,
    CloseWindow,
    CloseWindowById(u64),
    FullscreenWindow,
    FullscreenWindowById(u64),
    ToggleWindowedFullscreen,
    ToggleWindowedFullscreenById(u64),
    FocusWindow(u64),
    FocusWindowInColumn(u8),
    FocusWindowPrevious,
    FocusColumnLeft,
    FocusColumnLeftUnderMouse,
    FocusColumnRight,
    FocusColumnRightUnderMouse,
    FocusColumnFirst,
    FocusColumnLast,
    FocusColumnRightOrFirst,
    FocusColumnLeftOrLast,
    FocusColumn(usize),
    FocusWindowOrMonitorUp,
    FocusWindowOrMonitorDown,
    FocusColumnOrMonitorLeft,
    FocusColumnOrMonitorRight,
    FocusWindowDown,
    FocusWindowUp,
    FocusWindowDownOrColumnLeft,
    FocusWindowDownOrColumnRight,
    FocusWindowUpOrColumnLeft,
    FocusWindowUpOrColumnRight,
    FocusWindowOrWorkspaceDown,
    FocusWindowOrWorkspaceUp,
    FocusWindowTop,
    FocusWindowBottom,
    FocusWindowDownOrTop,
    FocusWindowUpOrBottom,
    MoveColumnLeft,
    MoveColumnRight,
    MoveColumnToFirst,
    MoveColumnToLast,
    MoveColumnLeftOrToMonitorLeft,
    MoveColumnRightOrToMonitorRight,
    MoveColumnToIndex(usize),
    MoveWindowDown,
    MoveWindowUp,
    MoveWindowLeft,
    MoveWindowRight,
    MoveWindowDownOrToWorkspaceDown,
    MoveWindowUpOrToWorkspaceUp,
    ConsumeOrExpelWindowLeft,
    ConsumeOrExpelWindowLeftById(u64),
    ConsumeOrExpelWindowRight,
    ConsumeOrExpelWindowRightById(u64),
    ConsumeWindowIntoColumn,
    ExpelWindowFromColumn,
    ToggleSplit,
    Preselect(Direction),
    PromoteWindow,
    SwapWindowLeft,
    SwapWindowRight,
    SwitchColumnDisplay,
    SetColumnDisplay(ColumnDisplay),
    CenterColumn,
    CenterWindow,
    CenterWindowById(u64),
    CenterVisibleColumns,
    FocusWorkspaceDown,
    FocusWorkspaceDownUnderMouse,
    FocusWorkspaceUp,
    FocusWorkspaceUpUnderMouse,
    FocusWorkspace(WorkspaceReference),
    FocusWorkspacePrevious,
    MoveWindowToWorkspaceDown(bool),
    MoveWindowToWorkspaceUp(bool),
    MoveWindowToWorkspace(
        WorkspaceReference,
        bool,
    ),
    MoveWindowToWorkspaceById {
        window_id: u64,
        reference: WorkspaceReference,
        focus: bool,
    },
    MoveColumnToWorkspaceDown(bool),
    MoveColumnToWorkspaceUp(bool),
    MoveColumnToWorkspace(
        WorkspaceReference,
        bool,
    ),
    MoveWorkspaceDown,
    MoveWorkspaceUp,
    MoveWorkspaceToIndex(usize),
    MoveWorkspaceToIndexByRef {
        new_idx: usize,
        reference: WorkspaceReference,
    },
    MoveWorkspaceToMonitorByRef {
        output_name: String,
        reference: WorkspaceReference,
    },
    MoveWorkspaceToMonitor(String),
    SetWorkspaceName(String),
    SetWorkspaceNameByRef {
        name: String,
        reference: WorkspaceReference,
    },
    UnsetWorkspaceName,
    UnsetWorkSpaceNameByRef(WorkspaceReference),
    FocusMonitorLeft,
    FocusMonitorRight,
    FocusMonitorDown,
    FocusMonitorUp,
    FocusMonitorPrevious,
    FocusMonitorNext,
    FocusMonitor(String),
    MoveWindowToMonitorLeft,
    MoveWindowToMonitorRight,
    MoveWindowToMonitorDown,
    MoveWindowToMonitorUp,
    MoveWindowToMonitorPrevious,
    MoveWindowToMonitorNext,
    MoveWindowToMonitor(String),
    MoveWindowToMonitorById {
        id: u64,
        output: String,
    },
    MoveColumnToMonitorLeft,
    MoveColumnToMonitorRight,
    MoveColumnToMonitorDown,
    MoveColumnToMonitorUp,
    MoveColumnToMonitorPrevious,
    MoveColumnToMonitorNext,
    MoveColumnToMonitor(String),
    SetWindowWidth(SizeChange),
    SetWindowWidthById {
        id: u64,
        change: SizeChange,
    },
    SetWindowHeight(SizeChange),
    SetWindowHeightById {
        id: u64,
        change: SizeChange,
    },
    ResetWindowHeight,
    ResetWindowHeightById(u64),
    SwitchPresetColumnWidth,
    SwitchPresetColumnWidthBack,
    SwitchPresetWindowWidth,
    SwitchPresetWindowWidthBack,
    SwitchPresetWindowWidthById(u64),
    SwitchPresetWindowWidthBackById(u64),
    SwitchPresetWindowHeight,
    SwitchPresetWindowHeightBack,
    SwitchPresetWindowHeightById(u64),
    SwitchPresetWindowHeightBackById(u64),
    MaximizeColumn,
    MaximizeWindowToEdges,
    MaximizeWindowToEdgesById(u64),
    SetColumnWidth(SizeChange),
    ExpandColumnToAvailableWidth,
    SwitchLayout(LayoutSwitchTarget),
    ShowHotkeyOverlay,
    MoveWorkspaceToMonitorLeft,
    MoveWorkspaceToMonitorRight,
    MoveWorkspaceToMonitorDown,
    MoveWorkspaceToMonitorUp,
    MoveWorkspaceToMonitorPrevious,
    MoveWorkspaceToMonitorNext,
    ToggleWindowFloating,
    ToggleWindowFloatingById(u64),
    MoveWindowToFloating,
    MoveWindowToFloatingById(u64),
    MoveWindowToTiling,
    MoveWindowToTilingById(u64),
    FocusFloating,
    FocusTiling,
    SwitchFocusBetweenFloatingAndTiling,
    MoveFloatingWindowById {
        id: Option<u64>,
        x: PositionChange,
        y: PositionChange,
    },
    ToggleWindowRuleOpacity,
    ToggleWindowRuleOpacityById(u64),
    SetDynamicCastWindow,
    SetDynamicCastWindowById(u64),
    SetDynamicCastMonitor(Option<String>),
    ClearDynamicCastTarget,
    StopCast(u64),
    ToggleOverview,
    OpenOverview,
    CloseOverview,
    ToggleWindowUrgent(u64),
    SetWindowUrgent(u64),
    UnsetWindowUrgent(u64),
    LoadConfigFile(Option<String>),
    MruAdvance {
        direction: MruDirection,
        scope: Option<MruScope>,
        filter: Option<MruFilter>,
    },
    MruConfirm,
    MruCancel,
    MruCloseCurrentWindow,
    MruFirst,
    MruLast,
    MruSetScope(MruScope),
    MruCycleScope,
}

impl From<ymir_ipc::Action> for Action {
    fn from(value: ymir_ipc::Action) -> Self {
        match value {
            ymir_ipc::Action::Quit { skip_confirmation } => Self::Quit(skip_confirmation),
            ymir_ipc::Action::PowerOffMonitors {} => Self::PowerOffMonitors,
            ymir_ipc::Action::PowerOnMonitors {} => Self::PowerOnMonitors,
            ymir_ipc::Action::Spawn { command } => Self::Spawn(command),
            ymir_ipc::Action::SpawnSh { command } => Self::SpawnSh(command),
            ymir_ipc::Action::DoScreenTransition { delay_ms } => Self::DoScreenTransition(delay_ms),
            ymir_ipc::Action::Screenshot {
                show_pointer: _,
                path,
            } => Self::Screenshot(path),
            ymir_ipc::Action::ScreenshotScreen {
                write_to_disk: _,
                show_pointer: _,
                path,
            } => Self::ScreenshotScreen(path),
            ymir_ipc::Action::ScreenshotWindow {
                id: None,
                write_to_disk: _,
                show_pointer: _,
                path,
            } => Self::ScreenshotWindow(path),
            ymir_ipc::Action::ScreenshotWindow {
                id: Some(id),
                write_to_disk,
                show_pointer,
                path,
            } => Self::ScreenshotWindowById {
                id,
                write_to_disk,
                show_pointer,
                path,
            },
            ymir_ipc::Action::ToggleKeyboardShortcutsInhibit {} => {
                Self::ToggleKeyboardShortcutsInhibit
            }
            ymir_ipc::Action::CloseWindow { id: None } => Self::CloseWindow,
            ymir_ipc::Action::CloseWindow { id: Some(id) } => Self::CloseWindowById(id),
            ymir_ipc::Action::FullscreenWindow { id: None } => Self::FullscreenWindow,
            ymir_ipc::Action::FullscreenWindow { id: Some(id) } => Self::FullscreenWindowById(id),
            ymir_ipc::Action::ToggleWindowedFullscreen { id: None } => {
                Self::ToggleWindowedFullscreen
            }
            ymir_ipc::Action::ToggleWindowedFullscreen { id: Some(id) } => {
                Self::ToggleWindowedFullscreenById(id)
            }
            ymir_ipc::Action::FocusWindow { id } => Self::FocusWindow(id),
            ymir_ipc::Action::FocusWindowInColumn { index } => Self::FocusWindowInColumn(index),
            ymir_ipc::Action::FocusWindowPrevious {} => Self::FocusWindowPrevious,
            ymir_ipc::Action::FocusColumnLeft {} => Self::FocusColumnLeft,
            ymir_ipc::Action::FocusColumnRight {} => Self::FocusColumnRight,
            ymir_ipc::Action::FocusColumnFirst {} => Self::FocusColumnFirst,
            ymir_ipc::Action::FocusColumnLast {} => Self::FocusColumnLast,
            ymir_ipc::Action::FocusColumnRightOrFirst {} => Self::FocusColumnRightOrFirst,
            ymir_ipc::Action::FocusColumnLeftOrLast {} => Self::FocusColumnLeftOrLast,
            ymir_ipc::Action::FocusColumn { index } => Self::FocusColumn(index),
            ymir_ipc::Action::FocusWindowOrMonitorUp {} => Self::FocusWindowOrMonitorUp,
            ymir_ipc::Action::FocusWindowOrMonitorDown {} => Self::FocusWindowOrMonitorDown,
            ymir_ipc::Action::FocusColumnOrMonitorLeft {} => Self::FocusColumnOrMonitorLeft,
            ymir_ipc::Action::FocusColumnOrMonitorRight {} => Self::FocusColumnOrMonitorRight,
            ymir_ipc::Action::FocusWindowDown {} => Self::FocusWindowDown,
            ymir_ipc::Action::FocusWindowUp {} => Self::FocusWindowUp,
            ymir_ipc::Action::FocusWindowDownOrColumnLeft {} => Self::FocusWindowDownOrColumnLeft,
            ymir_ipc::Action::FocusWindowDownOrColumnRight {} => Self::FocusWindowDownOrColumnRight,
            ymir_ipc::Action::FocusWindowUpOrColumnLeft {} => Self::FocusWindowUpOrColumnLeft,
            ymir_ipc::Action::FocusWindowUpOrColumnRight {} => Self::FocusWindowUpOrColumnRight,
            ymir_ipc::Action::FocusWindowOrWorkspaceDown {} => Self::FocusWindowOrWorkspaceDown,
            ymir_ipc::Action::FocusWindowOrWorkspaceUp {} => Self::FocusWindowOrWorkspaceUp,
            ymir_ipc::Action::FocusWindowTop {} => Self::FocusWindowTop,
            ymir_ipc::Action::FocusWindowBottom {} => Self::FocusWindowBottom,
            ymir_ipc::Action::FocusWindowDownOrTop {} => Self::FocusWindowDownOrTop,
            ymir_ipc::Action::FocusWindowUpOrBottom {} => Self::FocusWindowUpOrBottom,
            ymir_ipc::Action::MoveColumnLeft {} => Self::MoveColumnLeft,
            ymir_ipc::Action::MoveColumnRight {} => Self::MoveColumnRight,
            ymir_ipc::Action::MoveColumnToFirst {} => Self::MoveColumnToFirst,
            ymir_ipc::Action::MoveColumnToLast {} => Self::MoveColumnToLast,
            ymir_ipc::Action::MoveColumnToIndex { index } => Self::MoveColumnToIndex(index),
            ymir_ipc::Action::MoveColumnLeftOrToMonitorLeft {} => {
                Self::MoveColumnLeftOrToMonitorLeft
            }
            ymir_ipc::Action::MoveColumnRightOrToMonitorRight {} => {
                Self::MoveColumnRightOrToMonitorRight
            }
            ymir_ipc::Action::MoveWindowDown {} => Self::MoveWindowDown,
            ymir_ipc::Action::MoveWindowUp {} => Self::MoveWindowUp,
            ymir_ipc::Action::MoveWindowLeft {} => Self::MoveWindowLeft,
            ymir_ipc::Action::MoveWindowRight {} => Self::MoveWindowRight,
            ymir_ipc::Action::MoveWindowDownOrToWorkspaceDown {} => {
                Self::MoveWindowDownOrToWorkspaceDown
            }
            ymir_ipc::Action::MoveWindowUpOrToWorkspaceUp {} => Self::MoveWindowUpOrToWorkspaceUp,
            ymir_ipc::Action::ConsumeOrExpelWindowLeft { id: None } => {
                Self::ConsumeOrExpelWindowLeft
            }
            ymir_ipc::Action::ConsumeOrExpelWindowLeft { id: Some(id) } => {
                Self::ConsumeOrExpelWindowLeftById(id)
            }
            ymir_ipc::Action::ConsumeOrExpelWindowRight { id: None } => {
                Self::ConsumeOrExpelWindowRight
            }
            ymir_ipc::Action::ConsumeOrExpelWindowRight { id: Some(id) } => {
                Self::ConsumeOrExpelWindowRightById(id)
            }
            ymir_ipc::Action::ConsumeWindowIntoColumn {} => Self::ConsumeWindowIntoColumn,
            ymir_ipc::Action::ExpelWindowFromColumn {} => Self::ExpelWindowFromColumn,
            ymir_ipc::Action::ToggleSplit {} => Self::ToggleSplit,
            ymir_ipc::Action::Preselect { direction } => {
                Self::Preselect(Direction::from(direction))
            }
            ymir_ipc::Action::PromoteWindow {} => Self::PromoteWindow,
            ymir_ipc::Action::SwapWindowRight {} => Self::SwapWindowRight,
            ymir_ipc::Action::SwapWindowLeft {} => Self::SwapWindowLeft,
            ymir_ipc::Action::SwitchColumnDisplay {} => Self::SwitchColumnDisplay,
            ymir_ipc::Action::SetColumnDisplay { display } => Self::SetColumnDisplay(display),
            ymir_ipc::Action::CenterColumn {} => Self::CenterColumn,
            ymir_ipc::Action::CenterWindow { id: None } => Self::CenterWindow,
            ymir_ipc::Action::CenterWindow { id: Some(id) } => Self::CenterWindowById(id),
            ymir_ipc::Action::CenterVisibleColumns {} => Self::CenterVisibleColumns,
            ymir_ipc::Action::FocusWorkspaceDown {} => Self::FocusWorkspaceDown,
            ymir_ipc::Action::FocusWorkspaceUp {} => Self::FocusWorkspaceUp,
            ymir_ipc::Action::FocusWorkspace { reference } => {
                Self::FocusWorkspace(WorkspaceReference::from(reference))
            }
            ymir_ipc::Action::FocusWorkspacePrevious {} => Self::FocusWorkspacePrevious,
            ymir_ipc::Action::MoveWindowToWorkspaceDown { focus } => {
                Self::MoveWindowToWorkspaceDown(focus)
            }
            ymir_ipc::Action::MoveWindowToWorkspaceUp { focus } => {
                Self::MoveWindowToWorkspaceUp(focus)
            }
            ymir_ipc::Action::MoveWindowToWorkspace {
                window_id: None,
                reference,
                focus,
            } => Self::MoveWindowToWorkspace(WorkspaceReference::from(reference), focus),
            ymir_ipc::Action::MoveWindowToWorkspace {
                window_id: Some(window_id),
                reference,
                focus,
            } => Self::MoveWindowToWorkspaceById {
                window_id,
                reference: WorkspaceReference::from(reference),
                focus,
            },
            ymir_ipc::Action::MoveColumnToWorkspaceDown { focus } => {
                Self::MoveColumnToWorkspaceDown(focus)
            }
            ymir_ipc::Action::MoveColumnToWorkspaceUp { focus } => {
                Self::MoveColumnToWorkspaceUp(focus)
            }
            ymir_ipc::Action::MoveColumnToWorkspace { reference, focus } => {
                Self::MoveColumnToWorkspace(WorkspaceReference::from(reference), focus)
            }
            ymir_ipc::Action::MoveWorkspaceDown {} => Self::MoveWorkspaceDown,
            ymir_ipc::Action::MoveWorkspaceUp {} => Self::MoveWorkspaceUp,
            ymir_ipc::Action::SetWorkspaceName {
                name,
                workspace: None,
            } => Self::SetWorkspaceName(name),
            ymir_ipc::Action::SetWorkspaceName {
                name,
                workspace: Some(reference),
            } => Self::SetWorkspaceNameByRef {
                name,
                reference: WorkspaceReference::from(reference),
            },
            ymir_ipc::Action::UnsetWorkspaceName { reference: None } => Self::UnsetWorkspaceName,
            ymir_ipc::Action::UnsetWorkspaceName {
                reference: Some(reference),
            } => Self::UnsetWorkSpaceNameByRef(WorkspaceReference::from(reference)),
            ymir_ipc::Action::FocusMonitorLeft {} => Self::FocusMonitorLeft,
            ymir_ipc::Action::FocusMonitorRight {} => Self::FocusMonitorRight,
            ymir_ipc::Action::FocusMonitorDown {} => Self::FocusMonitorDown,
            ymir_ipc::Action::FocusMonitorUp {} => Self::FocusMonitorUp,
            ymir_ipc::Action::FocusMonitorPrevious {} => Self::FocusMonitorPrevious,
            ymir_ipc::Action::FocusMonitorNext {} => Self::FocusMonitorNext,
            ymir_ipc::Action::FocusMonitor { output } => Self::FocusMonitor(output),
            ymir_ipc::Action::MoveWindowToMonitorLeft {} => Self::MoveWindowToMonitorLeft,
            ymir_ipc::Action::MoveWindowToMonitorRight {} => Self::MoveWindowToMonitorRight,
            ymir_ipc::Action::MoveWindowToMonitorDown {} => Self::MoveWindowToMonitorDown,
            ymir_ipc::Action::MoveWindowToMonitorUp {} => Self::MoveWindowToMonitorUp,
            ymir_ipc::Action::MoveWindowToMonitorPrevious {} => Self::MoveWindowToMonitorPrevious,
            ymir_ipc::Action::MoveWindowToMonitorNext {} => Self::MoveWindowToMonitorNext,
            ymir_ipc::Action::MoveWindowToMonitor { id: None, output } => {
                Self::MoveWindowToMonitor(output)
            }
            ymir_ipc::Action::MoveWindowToMonitor {
                id: Some(id),
                output,
            } => Self::MoveWindowToMonitorById { id, output },
            ymir_ipc::Action::MoveColumnToMonitorLeft {} => Self::MoveColumnToMonitorLeft,
            ymir_ipc::Action::MoveColumnToMonitorRight {} => Self::MoveColumnToMonitorRight,
            ymir_ipc::Action::MoveColumnToMonitorDown {} => Self::MoveColumnToMonitorDown,
            ymir_ipc::Action::MoveColumnToMonitorUp {} => Self::MoveColumnToMonitorUp,
            ymir_ipc::Action::MoveColumnToMonitorPrevious {} => Self::MoveColumnToMonitorPrevious,
            ymir_ipc::Action::MoveColumnToMonitorNext {} => Self::MoveColumnToMonitorNext,
            ymir_ipc::Action::MoveColumnToMonitor { output } => Self::MoveColumnToMonitor(output),
            ymir_ipc::Action::SetWindowWidth { id: None, change } => Self::SetWindowWidth(change),
            ymir_ipc::Action::SetWindowWidth {
                id: Some(id),
                change,
            } => Self::SetWindowWidthById { id, change },
            ymir_ipc::Action::SetWindowHeight { id: None, change } => Self::SetWindowHeight(change),
            ymir_ipc::Action::SetWindowHeight {
                id: Some(id),
                change,
            } => Self::SetWindowHeightById { id, change },
            ymir_ipc::Action::ResetWindowHeight { id: None } => Self::ResetWindowHeight,
            ymir_ipc::Action::ResetWindowHeight { id: Some(id) } => Self::ResetWindowHeightById(id),
            ymir_ipc::Action::SwitchPresetColumnWidth {} => Self::SwitchPresetColumnWidth,
            ymir_ipc::Action::SwitchPresetColumnWidthBack {} => Self::SwitchPresetColumnWidthBack,
            ymir_ipc::Action::SwitchPresetWindowWidth { id: None } => Self::SwitchPresetWindowWidth,
            ymir_ipc::Action::SwitchPresetWindowWidthBack { id: None } => {
                Self::SwitchPresetWindowWidthBack
            }
            ymir_ipc::Action::SwitchPresetWindowWidth { id: Some(id) } => {
                Self::SwitchPresetWindowWidthById(id)
            }
            ymir_ipc::Action::SwitchPresetWindowWidthBack { id: Some(id) } => {
                Self::SwitchPresetWindowWidthBackById(id)
            }
            ymir_ipc::Action::SwitchPresetWindowHeight { id: None } => {
                Self::SwitchPresetWindowHeight
            }
            ymir_ipc::Action::SwitchPresetWindowHeightBack { id: None } => {
                Self::SwitchPresetWindowHeightBack
            }
            ymir_ipc::Action::SwitchPresetWindowHeight { id: Some(id) } => {
                Self::SwitchPresetWindowHeightById(id)
            }
            ymir_ipc::Action::SwitchPresetWindowHeightBack { id: Some(id) } => {
                Self::SwitchPresetWindowHeightBackById(id)
            }
            ymir_ipc::Action::MaximizeColumn {} => Self::MaximizeColumn,
            ymir_ipc::Action::MaximizeWindowToEdges { id: None } => Self::MaximizeWindowToEdges,
            ymir_ipc::Action::MaximizeWindowToEdges { id: Some(id) } => {
                Self::MaximizeWindowToEdgesById(id)
            }
            ymir_ipc::Action::SetColumnWidth { change } => Self::SetColumnWidth(change),
            ymir_ipc::Action::ExpandColumnToAvailableWidth {} => Self::ExpandColumnToAvailableWidth,
            ymir_ipc::Action::SwitchLayout { layout } => Self::SwitchLayout(layout),
            ymir_ipc::Action::ShowHotkeyOverlay {} => Self::ShowHotkeyOverlay,
            ymir_ipc::Action::MoveWorkspaceToMonitorLeft {} => Self::MoveWorkspaceToMonitorLeft,
            ymir_ipc::Action::MoveWorkspaceToMonitorRight {} => Self::MoveWorkspaceToMonitorRight,
            ymir_ipc::Action::MoveWorkspaceToMonitorDown {} => Self::MoveWorkspaceToMonitorDown,
            ymir_ipc::Action::MoveWorkspaceToMonitorUp {} => Self::MoveWorkspaceToMonitorUp,
            ymir_ipc::Action::MoveWorkspaceToMonitorPrevious {} => {
                Self::MoveWorkspaceToMonitorPrevious
            }
            ymir_ipc::Action::MoveWorkspaceToIndex {
                index,
                reference: Some(reference),
            } => Self::MoveWorkspaceToIndexByRef {
                new_idx: index,
                reference: WorkspaceReference::from(reference),
            },
            ymir_ipc::Action::MoveWorkspaceToIndex {
                index,
                reference: None,
            } => Self::MoveWorkspaceToIndex(index),
            ymir_ipc::Action::MoveWorkspaceToMonitor {
                output,
                reference: Some(reference),
            } => Self::MoveWorkspaceToMonitorByRef {
                output_name: output,
                reference: WorkspaceReference::from(reference),
            },
            ymir_ipc::Action::MoveWorkspaceToMonitor {
                output,
                reference: None,
            } => Self::MoveWorkspaceToMonitor(output),
            ymir_ipc::Action::MoveWorkspaceToMonitorNext {} => Self::MoveWorkspaceToMonitorNext,
            ymir_ipc::Action::ToggleDebugTint {} => Self::ToggleDebugTint,
            ymir_ipc::Action::DebugToggleOpaqueRegions {} => Self::DebugToggleOpaqueRegions,
            ymir_ipc::Action::DebugToggleDamage {} => Self::DebugToggleDamage,
            ymir_ipc::Action::ToggleWindowFloating { id: None } => Self::ToggleWindowFloating,
            ymir_ipc::Action::ToggleWindowFloating { id: Some(id) } => {
                Self::ToggleWindowFloatingById(id)
            }
            ymir_ipc::Action::MoveWindowToFloating { id: None } => Self::MoveWindowToFloating,
            ymir_ipc::Action::MoveWindowToFloating { id: Some(id) } => {
                Self::MoveWindowToFloatingById(id)
            }
            ymir_ipc::Action::MoveWindowToTiling { id: None } => Self::MoveWindowToTiling,
            ymir_ipc::Action::MoveWindowToTiling { id: Some(id) } => {
                Self::MoveWindowToTilingById(id)
            }
            ymir_ipc::Action::FocusFloating {} => Self::FocusFloating,
            ymir_ipc::Action::FocusTiling {} => Self::FocusTiling,
            ymir_ipc::Action::SwitchFocusBetweenFloatingAndTiling {} => {
                Self::SwitchFocusBetweenFloatingAndTiling
            }
            ymir_ipc::Action::MoveFloatingWindow { id, x, y } => {
                Self::MoveFloatingWindowById { id, x, y }
            }
            ymir_ipc::Action::ToggleWindowRuleOpacity { id: None } => Self::ToggleWindowRuleOpacity,
            ymir_ipc::Action::ToggleWindowRuleOpacity { id: Some(id) } => {
                Self::ToggleWindowRuleOpacityById(id)
            }
            ymir_ipc::Action::SetDynamicCastWindow { id: None } => Self::SetDynamicCastWindow,
            ymir_ipc::Action::SetDynamicCastWindow { id: Some(id) } => {
                Self::SetDynamicCastWindowById(id)
            }
            ymir_ipc::Action::SetDynamicCastMonitor { output } => {
                Self::SetDynamicCastMonitor(output)
            }
            ymir_ipc::Action::ClearDynamicCastTarget {} => Self::ClearDynamicCastTarget,
            ymir_ipc::Action::StopCast { session_id } => Self::StopCast(session_id),
            ymir_ipc::Action::ToggleOverview {} => Self::ToggleOverview,
            ymir_ipc::Action::OpenOverview {} => Self::OpenOverview,
            ymir_ipc::Action::CloseOverview {} => Self::CloseOverview,
            ymir_ipc::Action::ToggleWindowUrgent { id } => Self::ToggleWindowUrgent(id),
            ymir_ipc::Action::SetWindowUrgent { id } => Self::SetWindowUrgent(id),
            ymir_ipc::Action::UnsetWindowUrgent { id } => Self::UnsetWindowUrgent(id),
            ymir_ipc::Action::LoadConfigFile { path } => Self::LoadConfigFile(path),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WorkspaceReference {
    Id(u64),
    Index(u8),
    Name(String),
}

impl From<WorkspaceReferenceArg> for WorkspaceReference {
    fn from(reference: WorkspaceReferenceArg) -> WorkspaceReference {
        match reference {
            WorkspaceReferenceArg::Id(id) => Self::Id(id),
            WorkspaceReferenceArg::Index(i) => Self::Index(i),
            WorkspaceReferenceArg::Name(n) => Self::Name(n),
        }
    }
}

impl FromStr for Key {
    type Err = miette::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut modifiers = Modifiers::empty();

        let mut split = s.split('+');
        let key = split.next_back().unwrap();

        for part in split {
            let part = part.trim();
            if part.eq_ignore_ascii_case("mod") {
                modifiers |= Modifiers::COMPOSITOR
            } else if part.eq_ignore_ascii_case("ctrl") || part.eq_ignore_ascii_case("control") {
                modifiers |= Modifiers::CTRL;
            } else if part.eq_ignore_ascii_case("shift") {
                modifiers |= Modifiers::SHIFT;
            } else if part.eq_ignore_ascii_case("alt") {
                modifiers |= Modifiers::ALT;
            } else if part.eq_ignore_ascii_case("super") || part.eq_ignore_ascii_case("win") {
                modifiers |= Modifiers::SUPER;
            } else if part.eq_ignore_ascii_case("iso_level3_shift")
                || part.eq_ignore_ascii_case("mod5")
            {
                modifiers |= Modifiers::ISO_LEVEL3_SHIFT;
            } else if part.eq_ignore_ascii_case("iso_level5_shift")
                || part.eq_ignore_ascii_case("mod3")
            {
                modifiers |= Modifiers::ISO_LEVEL5_SHIFT;
            } else {
                return Err(miette!("invalid modifier: {part}"));
            }
        }

        let trigger = if key.eq_ignore_ascii_case("MouseLeft") {
            Trigger::MouseLeft
        } else if key.eq_ignore_ascii_case("MouseRight") {
            Trigger::MouseRight
        } else if key.eq_ignore_ascii_case("MouseMiddle") {
            Trigger::MouseMiddle
        } else if key.eq_ignore_ascii_case("MouseBack") {
            Trigger::MouseBack
        } else if key.eq_ignore_ascii_case("MouseForward") {
            Trigger::MouseForward
        } else if key.eq_ignore_ascii_case("WheelScrollDown") {
            Trigger::WheelScrollDown
        } else if key.eq_ignore_ascii_case("WheelScrollUp") {
            Trigger::WheelScrollUp
        } else if key.eq_ignore_ascii_case("WheelScrollLeft") {
            Trigger::WheelScrollLeft
        } else if key.eq_ignore_ascii_case("WheelScrollRight") {
            Trigger::WheelScrollRight
        } else if key.eq_ignore_ascii_case("TouchpadScrollDown") {
            Trigger::TouchpadScrollDown
        } else if key.eq_ignore_ascii_case("TouchpadScrollUp") {
            Trigger::TouchpadScrollUp
        } else if key.eq_ignore_ascii_case("TouchpadScrollLeft") {
            Trigger::TouchpadScrollLeft
        } else if key.eq_ignore_ascii_case("TouchpadScrollRight") {
            Trigger::TouchpadScrollRight
        } else if key.eq_ignore_ascii_case("TabletStylusButton1") {
            Trigger::TabletStylusButton1
        } else if key.eq_ignore_ascii_case("TabletStylusButton2") {
            Trigger::TabletStylusButton2
        } else if key.eq_ignore_ascii_case("TabletStylusButton3") {
            Trigger::TabletStylusButton3
        } else {
            let mut keysym = keysym_from_name(key, KEYSYM_CASE_INSENSITIVE);
            // The keyboard event handling code can receive either
            // XF86ScreenSaver or XF86Screensaver, because there is no
            // case mapping defined between these keysyms. If we just
            // use the case-insensitive version of keysym_from_name it
            // is not possible to bind the uppercase version, because the
            // case-insensitive match prefers the lowercase version when
            // there is a choice.
            //
            // Therefore, when we match this key with the initial
            // case-insensitive match we try a further case-sensitive match
            // (so that either key can be bound). If that fails, we change
            // to the uppercase version because:
            //
            // - A comment in xkb_keysym_from_name (in libxkbcommon) tells us that the uppercase
            //   version is the "best" of the two. [0]
            // - The xkbcommon crate only has a constant for ScreenSaver. [1]
            //
            // [0]: https://github.com/xkbcommon/libxkbcommon/blob/45a118d5325b051343b4b174f60c1434196fa7d4/src/keysym.c#L276
            // [1]: https://docs.rs/xkbcommon/latest/xkbcommon/xkb/keysyms/index.html#:~:text=KEY%5FXF86ScreenSaver
            //
            // See https://lab.braxton.onl/braxton/ymir/issues/1969
            if keysym == Keysym::XF86_Screensaver {
                keysym = keysym_from_name(key, KEYSYM_NO_FLAGS);
                if keysym.raw() == KEY_NoSymbol {
                    keysym = Keysym::XF86_ScreenSaver;
                }
            }
            if keysym.raw() == KEY_NoSymbol {
                return Err(miette!("invalid key: {key}"));
            }
            Trigger::Keysym(keysym)
        };

        Ok(Key { trigger, modifiers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_xf86_screensaver() {
        assert_eq!(
            "XF86ScreenSaver".parse::<Key>().unwrap(),
            Key {
                trigger: Trigger::Keysym(Keysym::XF86_ScreenSaver),
                modifiers: Modifiers::empty(),
            },
        );
        assert_eq!(
            "XF86Screensaver".parse::<Key>().unwrap(),
            Key {
                trigger: Trigger::Keysym(Keysym::XF86_Screensaver),
                modifiers: Modifiers::empty(),
            }
        );
        assert_eq!(
            "xf86screensaver".parse::<Key>().unwrap(),
            Key {
                trigger: Trigger::Keysym(Keysym::XF86_ScreenSaver),
                modifiers: Modifiers::empty(),
            }
        );
    }

    #[test]
    fn parse_iso_level_shifts() {
        assert_eq!(
            "ISO_Level3_Shift+A".parse::<Key>().unwrap(),
            Key {
                trigger: Trigger::Keysym(Keysym::a),
                modifiers: Modifiers::ISO_LEVEL3_SHIFT
            },
        );
        assert_eq!(
            "Mod5+A".parse::<Key>().unwrap(),
            Key {
                trigger: Trigger::Keysym(Keysym::a),
                modifiers: Modifiers::ISO_LEVEL3_SHIFT
            },
        );

        assert_eq!(
            "ISO_Level5_Shift+A".parse::<Key>().unwrap(),
            Key {
                trigger: Trigger::Keysym(Keysym::a),
                modifiers: Modifiers::ISO_LEVEL5_SHIFT
            },
        );
        assert_eq!(
            "Mod3+A".parse::<Key>().unwrap(),
            Key {
                trigger: Trigger::Keysym(Keysym::a),
                modifiers: Modifiers::ISO_LEVEL5_SHIFT
            },
        );
    }
}
