//! Dwindle column layout engine.
//!
//! This module implements the Ymir *Dwindle* column model: instead of a rigid linear stack of
//! windows inside each column on the infinite horizontal tape, every column slot is a recursive
//! binary-split container. New windows split the focused window instead of stacking linearly.
//!
//! The core data structure is a binary tree of [`Split`] nodes whose leaves are windows. Each
//! `Split` divides its region either horizontally (children stacked top-to-bottom) or vertically
//! (children arranged side-by-side) at a configurable ratio. The tree is generic over the leaf
//! type so that it stays fully unit-testable without dragging in any compositor types.
//!
//! The engine provides:
//!
//! * **Dynamic splitting** — [`DwindleTree::open_new`] slices the active leaf's region based on
//!   the current region's width-to-height ratio, so tall narrow cells keep stacking tiles while
//!   wide cells split side-by-side.
//! * **Preselection** — [`DwindleTree::preselect`] sets a one-time directional override for where
//!   the next spawned window will slice the active leaf.
//! * **`togglesplit`** — [`DwindleTree::toggle_split`] flips the split orientation of the active
//!   node's local container.
//! * **Expel / consume / promote** — [`DwindleTree::expel`] pulls a leaf (window) out of the tree
//!   while collapsing the vacated container; [`DwindleTree::consume`] makes the focused leaf absorb
//!   the region of its sibling subtree, and [`DwindleTree::promote`] moves a window to the head of
//!   the tree.
//! * **Geometry solving** — [`DwindleTree::leaf_rects`] partitions a bounding region into
//!   per-leaf rectangles respecting per-node ratios and gaps.

use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::utils::ResizeEdge;

/// The axis along which a [`Split`] divides its region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
    /// Children are stacked top-to-bottom.
    Horizontal,
    /// Children are arranged left-to-right.
    Vertical,
}

/// The spatial side a new window will take when slicing a node.
///
/// The side also implies the split axis of the new region: `Top`/`Bottom` produce a horizontal
/// (stacked) split, while `Left`/`Right` produce a vertical (side-by-side) split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitSide {
    Top,
    Bottom,
    Left,
    Right,
}

impl SplitSide {
    /// Returns the axis implied by this side.
    pub fn axis(self) -> SplitAxis {
        match self {
            Self::Top | Self::Bottom => SplitAxis::Horizontal,
            Self::Left | Self::Right => SplitAxis::Vertical,
        }
    }

    /// Returns which child slot this side maps to.
    fn child(self) -> Child {
        match self {
            Self::Top | Self::Left => Child::First,
            Self::Bottom | Self::Right => Child::Second,
        }
    }
}

/// Identifies one of the two children of a [`Split`].
///
/// * `First` is the top child of a horizontal split and the left child of a vertical split.
/// * `Second` is the bottom child of a horizontal split and the right child of a vertical split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Child {
    First,
    Second,
}

/// Path to a node in the tree, as a list of child choices from the root.
///
/// The empty path points at the root. A leaf path is a `LeafPath` whose every element is either
/// `First` or `Second`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LeafPath(Vec<Child>);

impl LeafPath {
    pub fn root() -> Self {
        Self(Vec::new())
    }

    fn push(self, child: Child) -> Self {
        let mut v = self.0;
        v.push(child);
        Self(v)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A node of the dwindle split tree.
#[derive(Debug)]
pub enum Node<T> {
    /// A single window.
    Leaf(T),
    /// A recursive binary split.
    Split {
        /// Orientation of the split.
        axis: SplitAxis,
        /// Visual ratio occupied by the `First` child (see [`DEFAULT_RATIO`]).
        ratio: f64,
        /// First child (top / left).
        first: Box<Node<T>>,
        /// Second child (bottom / right).
        second: Box<Node<T>>,
    },
}

/// Node ratio: visual fraction of the box given to the `First` child is `ratio / 2`, so `1.0`
/// is an even split. Ratios above `1.0` grow the `First` child at the expense of `Second`
/// (mirroring Hyprland's dwindle `splitRatio`), so a ratio of `0.5` and `1.5` are mirror images.
pub const DEFAULT_RATIO: f64 = 1.0;

/// Minimum ratio enforced when adjusting a split's ratio interactively.
pub const MIN_RATIO: f64 = 0.1;

/// Maximum ratio enforced when adjusting a split's ratio interactively.
pub const MAX_RATIO: f64 = 1.9;

/// Where a newly opened window is placed when no explicit side is preselected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ForceSplit {
    /// Pick the axis by the focused leaf's aspect ratio; the new window takes the freed
    /// bottom/right half (`Second`). With [`DwindleOptions::smart_split`] the split direction
    /// follows the cursor instead.
    #[default]
    Auto,
    /// Always place the new window in the top/left half (`First`).
    First,
    /// Always place the new window in the bottom/right half (`Second`).
    Second,
}

/// Configuration that shapes how a dwindle tree slices regions. Kept engine-native so the module
/// stays import-light and purely unit-testable; the compositor maps its config onto this.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DwindleOptions {
    /// Default ratio of a freshly created split (`1.0` is an even split).
    pub default_split_ratio: f64,
    /// Favor the newly opened window when splitting (Hyprland's `split_bias`).
    pub split_bias: bool,
    /// How the placement of a new window is chosen.
    pub force_split: ForceSplit,
    /// Keep each container's split orientation once chosen rather than re-evaluating it by the
    /// current region aspect ratio on every resize.
    pub preserve_split: bool,
    /// Multiplier used when deciding whether to split left/right vs top/bottom by aspect ratio.
    pub split_width_multiplier: f64,
    /// Choose the split direction by cursor position.
    pub smart_split: bool,
    /// Keep a preselected direction active for all subsequent windows until reset.
    pub permanent_direction_override: bool,
}

impl Default for DwindleOptions {
    fn default() -> Self {
        Self {
            default_split_ratio: DEFAULT_RATIO,
            split_bias: false,
            force_split: ForceSplit::Auto,
            preserve_split: false,
            split_width_multiplier: 1.,
            smart_split: false,
            permanent_direction_override: false,
        }
    }
}

impl<T> Node<T> {
    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf(_))
    }

    pub fn leaf_value(&self) -> Option<&T> {
        match self {
            Self::Leaf(v) => Some(v),
            Self::Split { .. } => None,
        }
    }

    fn leaf_count(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }
}

/// A binary-split (dwindle) tree of windows.
#[derive(Debug)]
pub struct DwindleTree<T> {
    root: Option<Node<T>>,
    active: Option<LeafPath>,
    preselect: Option<SplitSide>,
    options: DwindleOptions,
}

impl<T> DwindleTree<T> {
    /// Creates an empty tree.
    pub fn new() -> Self {
        Self {
            root: None,
            active: None,
            preselect: None,
            options: DwindleOptions::default(),
        }
    }

    /// Creates a tree with a single leaf, which is also the active leaf.
    pub fn single(value: T) -> Self {
        Self {
            root: Some(Node::Leaf(value)),
            active: Some(LeafPath::root()),
            preselect: None,
            options: DwindleOptions::default(),
        }
    }

    /// Returns a reference to the tree's layout options.
    pub fn options(&self) -> &DwindleOptions {
        &self.options
    }

    /// Replaces the tree's layout options.
    pub fn set_options(&mut self, options: DwindleOptions) {
        self.options = options;
    }

    /// Returns the number of leaves (windows) in the tree.
    pub fn len(&self) -> usize {
        self.root.as_ref().map(Node::leaf_count).unwrap_or(0)
    }

    /// Returns whether the tree has no leaves.
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Returns the path of the active leaf, if any.
    pub fn active(&self) -> Option<&LeafPath> {
        self.active.as_ref()
    }

    /// Returns the value of the active leaf, if any.
    pub fn active_value(&self) -> Option<&T> {
        let path = self.active.as_ref()?;
        self.leaf(path)
    }

    /// Sets the active leaf to the leaf at `path`, if it exists.
    ///
    /// Returns whether the path pointed at a leaf.
    pub fn set_active(&mut self, path: &LeafPath) -> bool {
        if self.leaf(path).is_none() {
            return false;
        }
        self.active = Some(path.clone());
        true
    }

    /// Applies `f` to every leaf value together with its depth-first position.
    ///
    /// Useful for re-establishing a "value == position" invariant after tree mutations.
    pub fn reindex(&mut self, f: impl Fn(&mut T, usize)) {
        let mut i = 0;
        reindex_node(self.root.as_mut(), &mut i, &f);
    }

    /// Returns the value at `path`.
    pub fn leaf(&self, path: &LeafPath) -> Option<&T> {
        self.leaf_impl(self.root.as_ref()?, path.0.as_slice())
    }

    fn leaf_impl<'a>(&'a self, node: &'a Node<T>, path: &[Child]) -> Option<&'a T> {
        match (node, path.first()) {
            (Node::Leaf(v), None) => Some(v),
            (Node::Leaf(_), Some(_)) => None,
            (Node::Split { .. }, None) => None,
            (Node::Split { first, .. }, Some(Child::First)) => self.leaf_impl(first, &path[1..]),
            (Node::Split { second, .. }, Some(Child::Second)) => self.leaf_impl(second, &path[1..]),
        }
    }

    /// Iterates over leaf values in depth-first (tree) order.
    pub fn leaves(&self) -> impl Iterator<Item = &T> + '_ {
        Leaves {
            stack: self.root.iter().collect(),
        }
    }

    /// Lists all leaf paths in depth-first order.
    pub fn leaf_paths(&self) -> Vec<LeafPath> {
        leaf_paths_of(&self.root)
    }

    /// Returns the path of the first leaf in depth-first order.
    pub fn first_leaf_path(&self) -> Option<LeafPath> {
        self.leaf_paths().into_iter().next()
    }

    /// Returns the path of the last leaf in depth-first order.
    pub fn last_leaf_path(&self) -> Option<LeafPath> {
        self.leaf_paths().into_iter().last()
    }

    /// Splits the tree into a leaf region partition of `content`, one rectangle per leaf, in
    /// depth-first order.
    ///
    /// Every consecutive pair of regions is separated by `gaps` logical pixels. The sum of all
    /// regions plus the interior gaps exactly recreates `content`.
    pub fn leaf_rects(
        &self,
        content: Rectangle<f64, Logical>,
        gaps: f64,
    ) -> Vec<(LeafPath, Rectangle<f64, Logical>)> {
        let mut out = Vec::new();
        self.solve_impl(self.root.as_ref(), content, gaps, LeafPath::root(), &mut out);
        out
    }

    fn solve_impl(
        &self,
        node: Option<&Node<T>>,
        rect: Rectangle<f64, Logical>,
        gaps: f64,
        path: LeafPath,
        out: &mut Vec<(LeafPath, Rectangle<f64, Logical>)>,
    ) {
        match node {
            None => (),
            Some(Node::Leaf(_)) => out.push((path, rect)),
            Some(Node::Split {
                axis,
                ratio,
                first,
                second,
            }) => {
                let (first_rect, second_rect) = split_rect(rect, *axis, *ratio, gaps);
                self.solve_impl(
                    Some(first),
                    first_rect,
                    gaps,
                    path.clone().push(Child::First),
                    out,
                );
                self.solve_impl(Some(second), second_rect, gaps, path.push(Child::Second), out);
            }
        }
    }

    /// Computes the rectangle currently occupied by the active leaf.
    pub fn active_rect(
        &self,
        content: Rectangle<f64, Logical>,
        gaps: f64,
    ) -> Option<Rectangle<f64, Logical>> {
        let rects = self.leaf_rects(content, gaps);
        let active = self.active.clone()?;
        rects.into_iter().find(|(path, _)| *path == active).map(|(_, r)| r)
    }

    /// Splits `content` into per-leaf rectangles keyed by the leaf *values*.
    ///
    /// Convenience used by the column geometry code to avoid path lookups.
    pub fn rects_by_value(
        &self,
        content: Rectangle<f64, Logical>,
        gaps: f64,
    ) -> Vec<(&T, Rectangle<f64, Logical>)> {
        let mut out = Vec::new();
        self.solve_values_impl(self.root.as_ref(), content, gaps, &mut out);
        out
    }

    fn solve_values_impl<'a>(
        &'a self,
        node: Option<&'a Node<T>>,
        rect: Rectangle<f64, Logical>,
        gaps: f64,
        out: &mut Vec<(&'a T, Rectangle<f64, Logical>)>,
    ) {
        match node {
            None => (),
            Some(Node::Leaf(v)) => out.push((v, rect)),
            Some(Node::Split {
                axis,
                ratio,
                first,
                second,
            }) => {
                let (first_rect, second_rect) = split_rect(rect, *axis, *ratio, gaps);
                self.solve_values_impl(Some(first), first_rect, gaps, out);
                self.solve_values_impl(Some(second), second_rect, gaps, out);
            }
        }
    }

    /// Opens a new window adjacent to the active leaf.
    ///
    /// The active leaf's region is split per the pending preselection (which persists when
    /// [`DwindleOptions::permanent_direction_override`] is set, else is consumed) or else per the
    /// configured split preferences. The new leaf becomes the active leaf.
    ///
    /// Returns the path of the newly opened leaf.
    pub fn open_new(&mut self, value: T, region: Size<f64, Logical>) -> LeafPath {
        let side_override = self.take_preselection_for_open();
        self.open_new_inner(value, side_override, None, region)
    }

    /// Like [`Self::open_new`], but uses `cursor` (in content coordinates) to pick the split
    /// direction when [`DwindleOptions::smart_split`] is enabled.
    pub fn open_new_at(
        &mut self,
        value: T,
        region: Size<f64, Logical>,
        cursor: Point<f64, Logical>,
    ) -> LeafPath {
        let side_override = self.take_preselection_for_open();
        self.open_new_inner(value, side_override, Some(cursor), region)
    }

    /// Like [`Self::open_new`], with an explicit side override instead of a preset.
    pub fn open_new_on(&mut self, value: T, side: SplitSide, region: Size<f64, Logical>) -> LeafPath {
        self.open_new_inner(value, Some(side), None, region)
    }

    /// Takes the pending preselection, keeping it if [`DwindleOptions::permanent_direction_override`]
    /// is set.
    fn take_preselection_for_open(&mut self) -> Option<SplitSide> {
        if self.options.permanent_direction_override {
            self.preselect
        } else {
            self.preselect.take()
        }
    }

    fn open_new_inner(
        &mut self,
        value: T,
        side_override: Option<SplitSide>,
        cursor: Option<Point<f64, Logical>>,
        region: Size<f64, Logical>,
    ) -> LeafPath {
        if self.root.is_none() {
            self.root = Some(Node::Leaf(value));
            let path = LeafPath::root();
            self.active = Some(path.clone());
            return path;
        }

        let active = self.active.clone().unwrap_or_else(LeafPath::root);

        let side = match side_override {
            Some(side) => side,
            None => {
                let rect = self
                    .active_rect(Rectangle::new(Point::from((0., 0.)), region), 0.)
                    .unwrap_or_else(|| Rectangle::new(Point::from((0., 0.)), region));
                self.resolve_default_side(rect, cursor)
            }
        };

        // Hyprland's `split_bias`: when the newly opened window gets the `First` half, flip the
        // default ratio so the *new* window is favoured whenever `default_split_ratio` isn't even.
        let mut new_ratio = clamp_ratio(self.options.default_split_ratio);
        if self.options.split_bias && side.child() == Child::First {
            new_ratio = clamp_ratio(2. - new_ratio);
        }

        let root = self.root.take().unwrap();
        let (new_root, new_path) = open_leaf_at(root, active.0.as_slice(), side, value, new_ratio);
        self.root = Some(new_root);
        self.active = Some(new_path.clone());
        new_path
    }

    /// Resolves the split side for a new window with no explicit preselection.
    ///
    /// Mirrors Hyprland's dwindle `addTarget`:
    /// * when `smart_split` is on and a cursor is available, the split direction follows the cursor
    ///   (dividing the active leaf into four triangles);
    /// * otherwise the axis is chosen by the active leaf's aspect ratio (with
    ///   [`DwindleOptions::split_width_multiplier`]) and the placement (`First`/`Second`) by
    ///   [`ForceSplit`]. The default (`Auto`) leaves the new window in the `Second` (bottom/right)
    ///   half.
    fn resolve_default_side(
        &self,
        active_rect: Rectangle<f64, Logical>,
        cursor: Option<Point<f64, Logical>>,
    ) -> SplitSide {
        if self.options.force_split == ForceSplit::Auto && self.options.smart_split {
            if let Some(cursor) = cursor {
                return smart_split_side(active_rect, cursor);
            }
        }

        let split_top =
            active_rect.size.h * self.options.split_width_multiplier > active_rect.size.w;
        let first_child = match self.options.force_split {
            ForceSplit::First => true,
            ForceSplit::Second | ForceSplit::Auto => false,
        };

        if split_top {
            if first_child {
                SplitSide::Top
            } else {
                SplitSide::Bottom
            }
        } else if first_child {
            SplitSide::Left
        } else {
            SplitSide::Right
        }
    }

    /// Replaces the leaf at `path` with `node`.
    pub fn replace_leaf(&mut self, path: &LeafPath, node: Node<T>) -> bool {
        let Some(root) = self.root.as_mut() else {
            return false;
        };
        replace_leaf_impl(root, &path.0, node)
    }

    /// Toggles (flips) the split orientation of the container directly holding the leaf at
    /// `path`.
    ///
    /// Returns whether a split was flipped.
    pub fn toggle_split(&mut self, path: &LeafPath) -> bool {
        let Some(root) = self.root.as_mut() else {
            return false;
        };
        toggle_split_impl(root, &path.0)
    }

    /// Sets a one-time directional override for the next [`Self::open_new`].
    pub fn preselect(&mut self, side: SplitSide) {
        self.preselect = Some(side);
    }

    /// Takes and clears any pending preselection.
    pub fn take_preselection(&mut self) -> Option<SplitSide> {
        self.preselect.take()
    }

    /// Returns whether a preselection is pending, without consuming it.
    pub fn pending_preselection(&self) -> Option<SplitSide> {
        self.preselect
    }

    /// Expels the leaf at `path` out of the tree, collapsing the vacated container so its sibling
    /// subtree takes over the whole region.
    ///
    /// Returns the expelled value, or `None` if the path did not point at a leaf.
    pub fn expel(&mut self, path: &LeafPath) -> Option<T> {
        let root = self.root.take()?;
        match remove_leaf(root, &path.0) {
            RemoveOutcome::Removed { subtree, value } => {
                self.root = subtree.map(|b| *b);
                let active_was_removed = self.active.as_ref().is_none_or(|a| a == path);
                if self.root.is_none() {
                    self.active = None;
                } else if active_was_removed {
                    self.active = self.leaf_paths().into_iter().next();
                }
                Some(value)
            }
            RemoveOutcome::Restore(node) => {
                self.root = Some(node);
                None
            }
        }
    }

    /// Removes the sibling subtree of the leaf at `path` so that the focused leaf absorbs the whole
    /// region of its container.
    ///
    /// Returns the values of all removed leaves, or `None` if the focused leaf had no sibling.
    pub fn consume(&mut self, path: &LeafPath) -> Option<Vec<T>> {
        let root = self.root.take()?;
        let (new_root, removed) = consume_leaf(root, &path.0);
        self.root = Some(*new_root);
        // The focused leaf now occupies the slot formerly held by its container, so its path
        // becomes the container's path.
        let new_active = path_without_last(path);
        self.active = Some(new_active);
        if removed.is_empty() {
            None
        } else {
            Some(removed)
        }
    }

    /// Moves the leaf at `path` to the head of the tree (its value ends up in the first leaf).
    ///
    /// Returns whether a move took place.
    pub fn promote(&mut self, path: &LeafPath) -> bool {
        let Some(head) = self.first_leaf_path() else {
            return false;
        };
        if head == *path {
            return false;
        }
        self.swap_leaves(path, &head);
        true
    }

    /// Swaps the values of the leaves at `a` and `b`.
    pub fn swap_leaves(&mut self, a: &LeafPath, b: &LeafPath) {
        if a == b {
            return;
        }
        if let Some(root) = self.root.as_mut() {
            swap_leaves_impl(root, &a.0, &b.0);
        }
    }

    /// Activates the leaf `step` positions after (or before, for negative steps) `from` in
    /// depth-first order.
    ///
    /// Returns the newly active path.
    pub fn focus_by(&mut self, from: &LeafPath, step: i32) -> Option<LeafPath> {
        let paths = self.leaf_paths();
        if paths.is_empty() {
            return None;
        }
        let idx = paths.iter().position(|p| p == from)?;
        let len = paths.len() as i32;
        let new = (idx as i32 + step).clamp(0, len - 1);
        let path = paths[new as usize].clone();
        self.active = Some(path.clone());
        Some(path)
    }

    /// Finds the spatially adjacent leaf in direction `dir`, using the leaf rectangles returned by
    /// [`Self::leaf_rects`] over `content`.
    ///
    /// A candidate must be immediately adjacent to `from` in that direction — its near edge must
    /// sit within a gap of `from`'s far edge (the divider between them) — and it must overlap
    /// `from` along the perpendicular axis. Among the candidates the one with the greatest
    /// perpendicular overlap wins (ties broken by closer distance, then DFS order). This mirrors
    /// Hyprland's dwindle directional focus: focus follows the window sharing a real divider, never
    /// jumping diagonally to a taller/wider neighbor just because its center lines up.
    ///
    /// Returns `None` when there is no leaf immediately in that direction.
    pub fn spatial_neighbor(
        &self,
        from: &LeafPath,
        dir: SpatialDir,
        content: Rectangle<f64, Logical>,
        gaps: f64,
    ) -> Option<LeafPath> {
        let rects = self.leaf_rects(content, gaps);
        let from_rect = rects.iter().find(|(p, _)| p == from)?.1;
        let tol = gaps + 1.0;

        let mut best: Option<(f64, f64, &LeafPath)> = None;
        for (path, rect) in &rects {
            if path == from {
                continue;
            }
            // Perpendicular overlap and the near-edge gap to `from`'s far edge, if the candidate is
            // the immediate neighbor in `dir`.
            let candidate = match dir {
                SpatialDir::Right => {
                    let gap_x = rect.loc.x - (from_rect.loc.x + from_rect.size.w);
                    if gap_x >= -tol && gap_x <= tol {
                        let overlap = vertical_overlap(from_rect, *rect);
                        (overlap > 0.).then_some((overlap, gap_x))
                    } else {
                        None
                    }
                }
                SpatialDir::Left => {
                    let gap_x = from_rect.loc.x - (rect.loc.x + rect.size.w);
                    if gap_x >= -tol && gap_x <= tol {
                        let overlap = vertical_overlap(from_rect, *rect);
                        (overlap > 0.).then_some((overlap, gap_x))
                    } else {
                        None
                    }
                }
                SpatialDir::Down => {
                    let gap_y = rect.loc.y - (from_rect.loc.y + from_rect.size.h);
                    if gap_y >= -tol && gap_y <= tol {
                        let overlap = horizontal_overlap(from_rect, *rect);
                        (overlap > 0.).then_some((overlap, gap_y))
                    } else {
                        None
                    }
                }
                SpatialDir::Up => {
                    let gap_y = from_rect.loc.y - (rect.loc.y + rect.size.h);
                    if gap_y >= -tol && gap_y <= tol {
                        let overlap = horizontal_overlap(from_rect, *rect);
                        (overlap > 0.).then_some((overlap, gap_y))
                    } else {
                        None
                    }
                }
            };

            let better = match (best, candidate) {
                (_, None) => false,
                (None, Some(_)) => true,
                (Some((c_overlap, c_dist, _)), Some((n_overlap, n_dist))) => {
                    // Prefer more overlap, then a closer divider; exact ties keep the first (DFS
                    // order) candidate.
                    n_overlap > c_overlap || (n_overlap == c_overlap && n_dist < c_dist)
                }
            };
            if better {
                if let Some((overlap, dist)) = candidate {
                    best = Some((overlap, dist, path));
                }
            }
        }

        best.map(|(_, _, path)| path.clone())
    }

    /// Adjusts the ratio of the split that directly contains the leaf at `path`, moving the ratio
    /// toward `delta`.
    pub fn adjust_ratio(&mut self, path: &LeafPath, delta: f64) -> bool {
        let Some(root) = self.root.as_mut() else {
            return false;
        };
        adjust_ratio_impl(root, &path.0, delta)
    }

    /// Returns the child slot (`First`/`Second`) that `path` occupies within the nearest ancestor
    /// split of `axis` (the deepest split with that orientation lying on the path to the leaf), if
    /// any.
    ///
    /// Used by interactive resize to decide which divider a dragged window can move.
    pub fn leaf_side_in_split(&self, path: &LeafPath, axis: SplitAxis) -> Option<Child> {
        leaf_side_in_split_impl(self.root.as_ref()?, &path.0, axis, None)
    }

    /// Moves the divider of the nearest ancestor split of `axis` that contains `path`, translating
    /// the divider by `delta_px` logical pixels.
    ///
    /// A positive `delta_px` grows the split's `First` child (rightward for a `Vertical` split,
    /// downward for a `Horizontal` one). The ratio is clamped to `MIN_RATIO`..=`MAX_RATIO`.
    /// Returns whether such an ancestor split was found and adjusted.
    pub fn adjust_ancestor_ratio(
        &mut self,
        path: &LeafPath,
        axis: SplitAxis,
        delta_px: f64,
        usable: f64,
    ) -> bool {
        if usable <= 0. || !delta_px.is_finite() {
            return false;
        }
        let Some(root) = self.root.as_mut() else {
            return false;
        };
        adjust_ancestor_ratio_impl(root, &path.0, axis, 2. * delta_px / usable)
    }

    /// Moves the divider that the leaf's `edge` lies on, if any.
    ///
    /// A leaf edge coincides with the divider of exactly one ancestor split: the deepest one on
    /// the path whose orientation matches the edge's axis and whose `First`/`Second` slot places
    /// the shared divider on that edge (a `First` child shares its right/bottom edge, a `Second`
    /// child shares its left/top edge). That split may sit at *any* ancestor depth, not just the
    /// nearest matching-axis split — so every real divider of the tree is draggable from the
    /// windows it separates, including the root divider seen from a deeply nested leaf.
    ///
    /// `delta_px` follows the pointer: a positive delta grows the split's `First` child along the
    /// edge's axis. `usable_w`/`usable_h` are the content sizes minus the seam. `min_w`/`min_h`
    /// report each leaf's minimum extent (width for a `Vertical` divider, height for a
    /// `Horizontal` one); the divider is clamped so that neither child subtree shrinks below the
    /// combined minimum of its leaves. If the leaf's mins already exceed the usable space the
    /// divider is left untouched. Returns whether a divider was found and moved.
    #[allow(clippy::too_many_arguments)]
    pub fn adjust_ratio_for_edge(
        &mut self,
        path: &LeafPath,
        edge: ResizeEdge,
        delta_px: f64,
        usable_w: f64,
        usable_h: f64,
        gaps: f64,
        min_w: &impl Fn(&T) -> f64,
        min_h: &impl Fn(&T) -> f64,
    ) -> bool {
        if path.is_empty() || !delta_px.is_finite() || usable_w <= 0. || usable_h <= 0. {
            return false;
        }
        let Some(root) = self.root.as_mut() else {
            return false;
        };
        adjust_ratio_for_edge_impl(
            root,
            &path.0,
            edge,
            delta_px,
            usable_w + gaps,
            usable_h + gaps,
            gaps,
            min_w,
            min_h,
        )
    }
}

fn reindex_node<T>(node: Option<&mut Node<T>>, i: &mut usize, f: &impl Fn(&mut T, usize)) {
    match node {
        Some(Node::Leaf(v)) => {
            f(v, *i);
            *i += 1;
        }
        Some(Node::Split { first, second, .. }) => {
            reindex_node(Some(first), i, f);
            reindex_node(Some(second), i, f);
        }
        None => (),
    }
}

impl<T> Default for DwindleTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over leaf values in depth-first order.
struct Leaves<'a, T> {
    stack: Vec<&'a Node<T>>,
}

impl<'a, T> Iterator for Leaves<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.stack.pop()?;
            match node {
                Node::Leaf(v) => return Some(v),
                Node::Split { first, second, .. } => {
                    self.stack.push(second);
                    self.stack.push(first);
                }
            }
        }
    }
}

/// Divides `rect` into two regions along the given axis at `ratio`, inserting a `gaps`-wide seam.
///
/// The `First` child's extent is floored to a whole logical pixel and the `Second` child gets the
/// remainder, so every shared divider lands on exactly one pixel boundary: the two leaves it
/// separates compute their adjoining edges from the same value, instead of each rounding a
/// fractional boundary independently.
fn split_rect(
    rect: Rectangle<f64, Logical>,
    axis: SplitAxis,
    ratio: f64,
    gaps: f64,
) -> (
    Rectangle<f64, Logical>,
    Rectangle<f64, Logical>,
) {
    match axis {
        SplitAxis::Horizontal => {
            // Clamp so a region smaller than the seam can't produce a negative split size or a
            // second child that escapes the region.
            let usable = (rect.size.h - gaps).max(0.);
            let first_h = (usable * ratio / 2.).floor().clamp(0., usable);
            let second_h = (usable - first_h).max(0.);
            let second_y = (rect.loc.y + first_h + gaps)
                .min(rect.loc.y + rect.size.h - second_h);
            let first = Rectangle::new(rect.loc, Size::from((rect.size.w, first_h)));
            let second = Rectangle::new(
                Point::from((rect.loc.x, second_y)),
                Size::from((rect.size.w, second_h)),
            );
            (first, second)
        }
        SplitAxis::Vertical => {
            let usable = (rect.size.w - gaps).max(0.);
            let first_w = (usable * ratio / 2.).floor().clamp(0., usable);
            let second_w = (usable - first_w).max(0.);
            let second_x = (rect.loc.x + first_w + gaps)
                .min(rect.loc.x + rect.size.w - second_w);
            let first = Rectangle::new(rect.loc, Size::from((first_w, rect.size.h)));
            let second = Rectangle::new(
                Point::from((second_x, rect.loc.y)),
                Size::from((second_w, rect.size.h)),
            );
            (first, second)
        }
    }
}

/// Picks the split side for `cursor` (in `region` coordinates) by dividing the active leaf into
/// four triangles about its center, mirroring Hyprland's `smart_split`.
///
/// A shallow cursor angle (|slope| < height/width) splits side-by-side toward the cursor's half;
/// a steep angle splits top/bottom toward the cursor's half.
fn smart_split_side(
    region: Rectangle<f64, Logical>,
    cursor: Point<f64, Logical>,
) -> SplitSide {
    let center_x = region.loc.x + region.size.w / 2.;
    let center_y = region.loc.y + region.size.h / 2.;
    let dx = cursor.x - center_x;
    let dy = cursor.y - center_y;
    // The cursor exactly over the region's center has no well-defined half; fall back to the
    // aspect-ratio default (a tall region splits top/bottom, a wide one side-by-side), placing the
    // new window in the `Second` (bottom/right) half as Hyprland's `Auto` placement does.
    if dx == 0. && dy == 0. {
        return if region.size.h > region.size.w {
            SplitSide::Bottom
        } else {
            SplitSide::Right
        };
    }

    let proportions = if region.size.w == 0. {
        1.
    } else {
        region.size.h / region.size.w
    };
    let slope = if dx == 0. {
        f64::INFINITY * dy.signum()
    } else {
        dy / dx
    };
    if slope.abs() < proportions {
        if dx > 0. {
            SplitSide::Right
        } else {
            SplitSide::Left
        }
    } else if dy > 0. {
        SplitSide::Bottom
    } else {
        SplitSide::Top
    }
}

/// Recursively opens a new leaf next to the leaf at `path`, rebuilding only the nodes along the
/// path.
///
/// `ratio` is applied to the freshly created split. Returns the new subtree and the path of the
/// newly inserted leaf.
fn open_leaf_at<T>(
    node: Node<T>,
    path: &[Child],
    side: SplitSide,
    value: T,
    ratio: f64,
) -> (Node<T>, LeafPath) {
    match (node, path.first()) {
        (Node::Leaf(old), None) => {
            let axis = side.axis();
            let new_child = side.child();
            let (first, second) = match new_child {
                Child::First => (
                    Box::new(Node::Leaf(value)),
                    Box::new(Node::Leaf(old)),
                ),
                Child::Second => (
                    Box::new(Node::Leaf(old)),
                    Box::new(Node::Leaf(value)),
                ),
            };
            (
                Node::Split {
                    axis,
                    ratio,
                    first,
                    second,
                },
                LeafPath(vec![new_child]),
            )
        }
        (Node::Leaf(old), Some(_)) => {
            // Path points deeper than the tree goes; clamp to the leaf itself.
            open_leaf_at(Node::Leaf(old), &[], side, value, ratio)
        }
        (Node::Split { axis, ratio, first, second }, None) => {
            // Path ended at a split; insert into the first child instead.
            let (new_first, tail) = open_leaf_at(*first, &[], side, value, ratio);
            (
                Node::Split {
                    axis,
                    ratio,
                    first: Box::new(new_first),
                    second,
                },
                LeafPath(vec![Child::First]).push_many(&tail),
            )
        }
        (Node::Split { axis, ratio, first, second }, Some(Child::First)) => {
            let (new_first, tail) = open_leaf_at(*first, &path[1..], side, value, ratio);
            (
                Node::Split {
                    axis,
                    ratio,
                    first: Box::new(new_first),
                    second,
                },
                LeafPath::prepend(Child::First, tail),
            )
        }
        (Node::Split { axis, ratio, first, second }, Some(Child::Second)) => {
            let (new_second, tail) = open_leaf_at(*second, &path[1..], side, value, ratio);
            (
                Node::Split {
                    axis,
                    ratio,
                    first,
                    second: Box::new(new_second),
                },
                LeafPath::prepend(Child::Second, tail),
            )
        }
    }
}

impl LeafPath {
    fn push_many(&self, other: &LeafPath) -> LeafPath {
        let mut v = self.0.clone();
        v.extend(other.0.iter().copied());
        LeafPath(v)
    }

    fn prepend(child: Child, mut tail: LeafPath) -> LeafPath {
        tail.0.insert(0, child);
        tail
    }
}

/// Outcome of a recursive `remove_leaf` traversal.
enum RemoveOutcome<T> {
    /// The targeted leaf was found and removed.
    Removed {
        /// The rebuilt subtree (None if the whole subtree vanished).
        subtree: Option<Box<Node<T>>>,
        /// The removed leaf value.
        value: T,
    },
    /// The path did not resolve to a leaf; the node is restored unchanged.
    Restore(Node<T>),
}

/// Removes the leaf at `path`, collapsing any container whose sibling takes over its place.
fn remove_leaf<T>(node: Node<T>, path: &[Child]) -> RemoveOutcome<T> {
    match (node, path.first()) {
        (Node::Leaf(v), None) => RemoveOutcome::Removed {
            subtree: None,
            value: v,
        },
        (Node::Leaf(v), Some(_)) => RemoveOutcome::Restore(Node::Leaf(v)),
        (Node::Split { axis, ratio, first, second }, None) => RemoveOutcome::Restore(
            Node::Split {
                axis,
                ratio,
                first,
                second,
            },
        ),
        (Node::Split { axis, ratio, first, second }, Some(child)) => {
            let (target, sibling) = match child {
                Child::First => (first, second),
                Child::Second => (second, first),
            };
            match remove_leaf(*target, &path[1..]) {
                RemoveOutcome::Restore(target) => {
                    let (first, second) = match child {
                        Child::First => (Box::new(target), sibling),
                        Child::Second => (sibling, Box::new(target)),
                    };
                    RemoveOutcome::Restore(Node::Split { axis, ratio, first, second })
                }
                RemoveOutcome::Removed { subtree, value } => {
                    // The leaf under `target` was removed. If `target` vanished entirely, the
                    // container collapses into the surviving sibling; otherwise it keeps both
                    // children.
                    let node = match (subtree, child) {
                        (None, _) => *sibling,
                        (Some(new_target), Child::First) => Node::Split {
                            axis,
                            ratio,
                            first: new_target,
                            second: sibling,
                        },
                        (Some(new_target), Child::Second) => Node::Split {
                            axis,
                            ratio,
                            first: sibling,
                            second: new_target,
                        },
                    };
                    RemoveOutcome::Removed {
                        subtree: Some(Box::new(node)),
                        value,
                    }
                }
            }
        }
    }
}

/// Collapses the container directly holding the leaf at `path`, draining the sibling subtree into
/// `removed` and leaving the focused leaf in place. The returned subtree always contains the
/// focused leaf.
fn consume_leaf<T>(node: Node<T>, path: &[Child]) -> (Box<Node<T>>, Vec<T>) {
    match node {
        Node::Leaf(_) => (Box::new(node), Vec::new()),
        Node::Split { axis, ratio, first, second } => match path.first() {
            None => (Box::new(Node::Split { axis, ratio, first, second }), Vec::new()),
            Some(child) => {
                let (target, sibling) = match child {
                    Child::First => (first, second),
                    Child::Second => (second, first),
                };

                if path.len() == 1 {
                    // The targeted child directly holds the focused leaf; drain the sibling and
                    // collapse the container into the focused leaf.
                    let removed = drain_leaves(*sibling);
                    (target, removed)
                } else {
                    let (new_target, removed) = consume_leaf(*target, &path[1..]);
                    let node = match child {
                        Child::First => Node::Split {
                            axis,
                            ratio,
                            first: new_target,
                            second: sibling,
                        },
                        Child::Second => Node::Split {
                            axis,
                            ratio,
                            first: sibling,
                            second: new_target,
                        },
                    };
                    (Box::new(node), removed)
                }
            }
        },
    }
}

fn drain_leaves<T>(node: Node<T>) -> Vec<T> {
    match node {
        Node::Leaf(v) => vec![v],
        Node::Split { first, second, .. } => {
            let mut out = drain_leaves(*first);
            out.append(&mut drain_leaves(*second));
            out
        }
    }
}

/// Returns `path` minus its final child choice (the empty path stays empty).
fn path_without_last(path: &LeafPath) -> LeafPath {
    let mut v = path.0.clone();
    if !v.is_empty() {
        v.pop();
    }
    LeafPath(v)
}

fn replace_leaf_impl<T>(node: &mut Node<T>, path: &[Child], new_node: Node<T>) -> bool {
    match path.first() {
        None => match node {
            Node::Leaf(_) => {
                *node = new_node;
                true
            }
            Node::Split { .. } => false,
        },
        Some(Child::First) => match node {
            Node::Split { first, .. } => replace_leaf_impl(first, &path[1..], new_node),
            Node::Leaf(_) => false,
        },
        Some(Child::Second) => match node {
            Node::Split { second, .. } => replace_leaf_impl(second, &path[1..], new_node),
            Node::Leaf(_) => false,
        },
    }
}

fn toggle_split_impl<T>(node: &mut Node<T>, path: &[Child]) -> bool {
    if path.is_empty() {
        return false;
    }
    let mut node = node;
    for c in &path[..path.len() - 1] {
        match (c, node) {
            (Child::First, Node::Split { first, .. }) => node = first,
            (Child::Second, Node::Split { second, .. }) => node = second,
            (_, Node::Leaf(_)) => return false,
        }
    }
    match node {
        Node::Split { axis, .. } => {
            *axis = match axis {
                SplitAxis::Horizontal => SplitAxis::Vertical,
                SplitAxis::Vertical => SplitAxis::Horizontal,
            };
            true
        }
        Node::Leaf(_) => false,
    }
}

fn swap_leaves_impl<T>(node: &mut Node<T>, a: &[Child], b: &[Child]) {
    if a.is_empty() || b.is_empty() {
        return;
    }
    if let Node::Split { first, second, .. } = node {
        let (da, db) = (a[0], b[0]);
        if da == db {
            if a[1..] == b[1..] {
                return;
            }
            let child = match da {
                Child::First => first,
                Child::Second => second,
            };
            swap_leaves_impl(child, &a[1..], &b[1..]);
            return;
        }

        let (fa, fb) = match (da, db) {
            (Child::First, Child::Second) => (first, second),
            (Child::Second, Child::First) => (second, first),
            _ => unreachable!("equal first-step children handled above"),
        };
        if let (Some(va), Some(vb)) = (leaf_value_mut_of(fa, &a[1..]), leaf_value_mut_of(fb, &b[1..]))
        {
            std::mem::swap(va, vb);
        }
    }
}

fn leaf_value_mut_of<'a, T>(node: &'a mut Node<T>, path: &[Child]) -> Option<&'a mut T> {
    match (node, path.first()) {
        (Node::Leaf(v), None) => Some(v),
        (Node::Leaf(_), Some(_)) => None,
        (Node::Split { .. }, None) => None,
        (Node::Split { first, .. }, Some(Child::First)) => {
            leaf_value_mut_of(first, &path[1..])
        }
        (Node::Split { second, .. }, Some(Child::Second)) => {
            leaf_value_mut_of(second, &path[1..])
        }
    }
}

fn adjust_ratio_impl<T>(node: &mut Node<T>, path: &[Child], delta: f64) -> bool {
    if path.is_empty() {
        return false;
    }
    let mut node = node;
    for c in &path[..path.len() - 1] {
        match (c, node) {
            (Child::First, Node::Split { first, .. }) => node = first,
            (Child::Second, Node::Split { second, .. }) => node = second,
            (_, Node::Leaf(_)) => return false,
        }
    }
    match node {
        Node::Split { ratio, .. } => {
            *ratio = clamp_ratio(*ratio + delta);
            true
        }
        Node::Leaf(_) => false,
    }
}

/// Clamps a ratio into the allowed `[MIN_RATIO, MAX_RATIO]` range.
fn clamp_ratio(ratio: f64) -> f64 {
    ratio.clamp(MIN_RATIO, MAX_RATIO)
}

/// Walks `path` from the root, tracking the child slot of the deepest split whose axis matches
/// `axis`. Returns that slot, or `acc` (which is `None` at the root) if none matches.
fn leaf_side_in_split_impl<T>(
    node: &Node<T>,
    path: &[Child],
    axis: SplitAxis,
    acc: Option<Child>,
) -> Option<Child> {
    match (node, path.first()) {
        (Node::Leaf(_), _) => acc,
        (Node::Split { axis: a, first, .. }, Some(Child::First)) => {
            let this = if *a == axis {
                Some(Child::First)
            } else {
                acc
            };
            leaf_side_in_split_impl(first, &path[1..], axis, this)
        }
        (Node::Split { axis: a, second, .. }, Some(Child::Second)) => {
            let this = if *a == axis {
                Some(Child::Second)
            } else {
                acc
            };
            leaf_side_in_split_impl(second, &path[1..], axis, this)
        }
        (Node::Split { .. }, None) => acc,
    }
}

/// Adjusts the ratio of the deepest split of `axis` on the path to the leaf, adding `delta` (the
/// ratio increment) to it. Descends fully first so the deepest matching split wins.
fn adjust_ancestor_ratio_impl<T>(
    node: &mut Node<T>,
    path: &[Child],
    axis: SplitAxis,
    delta: f64,
) -> bool {
    match (node, path.first()) {
        (Node::Split { axis: a, ratio, first, second }, Some(child)) => {
            let deeper = match child {
                Child::First => adjust_ancestor_ratio_impl(first, &path[1..], axis, delta),
                Child::Second => adjust_ancestor_ratio_impl(second, &path[1..], axis, delta),
            };
            if deeper {
                return true;
            }
            if *a == axis {
                *ratio = clamp_ratio(*ratio + delta);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Total minimum extent of `node`'s leaves along `axis`. Children partition `axis` when the node
/// splits along it (mins sum) and stack perpendicular to it (the largest min wins).
fn subtree_min<T>(
    node: &Node<T>,
    axis: SplitAxis,
    min_w: &impl Fn(&T) -> f64,
    min_h: &impl Fn(&T) -> f64,
) -> f64 {
    match node {
        Node::Leaf(v) => match axis {
            SplitAxis::Vertical => min_w(v),
            SplitAxis::Horizontal => min_h(v),
        },
        Node::Split {
            axis: a,
            first,
            second,
            ..
        } => {
            let f = subtree_min(first, axis, min_w, min_h);
            let s = subtree_min(second, axis, min_w, min_h);
            if *a == axis {
                f + s
            } else {
                f.max(s)
            }
        }
    }
}

/// Moves the divider of the node on `path` whose interior edge coincides with `edge`, if any.
///
/// The deepest matching split on the path wins (descend first, then claim this node), but a node
/// only owns `edge` when the leaf's slot on its axis places the shared divider on that edge — so
/// a shallow split can still move even when deeper splits of different orientation sit in
/// between. The ratio is clamped so neither child subtree shrinks below the combined minimum of
/// its leaves (empty clamp range leaves the ratio untouched).
///
/// `box_w`/`box_h` are the current node's own rectangle extent (the root split starts with the
/// full content size). The pixel-to-ratio conversion and the minimum-ratio clamp each use this
/// *local* extent, so a divider of a deeply nested split follows the pointer 1:1 instead of being
/// scaled (and clamped) against the whole column's size — mirroring Hyprland, which scales by the
/// split's own `box`. Child boxes are derived from the parent's box as the recursion descends.
#[allow(clippy::too_many_arguments)]
fn adjust_ratio_for_edge_impl<T>(
    node: &mut Node<T>,
    path: &[Child],
    edge: ResizeEdge,
    delta_px: f64,
    box_w: f64,
    box_h: f64,
    gaps: f64,
    min_w: &impl Fn(&T) -> f64,
    min_h: &impl Fn(&T) -> f64,
) -> bool {
    let Node::Split {
        axis,
        ratio,
        first,
        second,
        ..
    } = node
    else {
        return false;
    };
    let Some(child) = path.first() else {
        return false;
    };
    let rest = &path[1..];
    if !rest.is_empty() {
        // Derive the matching child's box from this node's box so the deeper split scales by its
        // own extent.
        let rect = Rectangle::new(Point::from((0., 0.)), Size::from((box_w, box_h)));
        let (child_w, child_h) = match *axis {
            SplitAxis::Vertical => {
                let (first_rect, second_rect) = split_rect(rect, SplitAxis::Vertical, *ratio, gaps);
                match child {
                    Child::First => (first_rect.size.w, first_rect.size.h),
                    Child::Second => (second_rect.size.w, second_rect.size.h),
                }
            }
            SplitAxis::Horizontal => {
                let (first_rect, second_rect) = split_rect(rect, SplitAxis::Horizontal, *ratio, gaps);
                match child {
                    Child::First => (first_rect.size.w, first_rect.size.h),
                    Child::Second => (second_rect.size.w, second_rect.size.h),
                }
            }
        };
        let deeper = match child {
            Child::First => adjust_ratio_for_edge_impl(
                &mut **first,
                rest,
                edge,
                delta_px,
                child_w,
                child_h,
                gaps,
                min_w,
                min_h,
            ),
            Child::Second => adjust_ratio_for_edge_impl(
                &mut **second,
                rest,
                edge,
                delta_px,
                child_w,
                child_h,
                gaps,
                min_w,
                min_h,
            ),
        };
        if deeper {
            return true;
        }
    }

    let slot_matches = match (*axis, child) {
        (SplitAxis::Vertical, Child::First) => edge.contains(ResizeEdge::RIGHT),
        (SplitAxis::Vertical, Child::Second) => edge.contains(ResizeEdge::LEFT),
        (SplitAxis::Horizontal, Child::First) => edge.contains(ResizeEdge::BOTTOM),
        (SplitAxis::Horizontal, Child::Second) => edge.contains(ResizeEdge::TOP),
    };
    if !slot_matches {
        return false;
    }

    let usable = match *axis {
        SplitAxis::Vertical => box_w - gaps,
        SplitAxis::Horizontal => box_h - gaps,
    };
    if usable <= 0. {
        return false;
    }

    let first_min = subtree_min(first, *axis, min_w, min_h);
    let second_min = subtree_min(second, *axis, min_w, min_h);
    // The First child's share is `ratio / 2`, so the divider pixel position is
    // `box * ratio / 2`; a 1px drag therefore changes `ratio` by `2 / usable`.
    let ratio_min = (2. * first_min / usable).max(MIN_RATIO);
    let ratio_max = (2. * (1. - second_min / usable)).min(MAX_RATIO);
    if ratio_min > ratio_max {
        return false;
    }

    *ratio = (*ratio + 2. * delta_px / usable).clamp(ratio_min, ratio_max);
    true
}

fn leaf_paths_of<T>(root: &Option<Node<T>>) -> Vec<LeafPath> {
    let mut out = Vec::new();
    let Some(root) = root else {
        return out;
    };
    let mut stack = vec![(root, LeafPath::root())];
    while let Some((node, path)) = stack.pop() {
        match node {
            Node::Leaf(_) => out.push(path),
            Node::Split { first, second, .. } => {
                stack.push((second, path.clone().push(Child::Second)));
                stack.push((first, path.push(Child::First)));
            }
        }
    }
    out
}

/// Length of the shared span of `a` and `b` along the vertical axis (how far their y-ranges
/// overlap). This is the perpendicular overlap relevant for left/right moves.
fn vertical_overlap(
    a: Rectangle<f64, Logical>,
    b: Rectangle<f64, Logical>,
) -> f64 {
    let top = f64::max(a.loc.y, b.loc.y);
    let bottom = f64::min(a.loc.y + a.size.h, b.loc.y + b.size.h);
    f64::max(0., bottom - top)
}

/// Length of the shared span of `a` and `b` along the horizontal axis (how far their x-ranges
/// overlap). This is the perpendicular overlap relevant for up/down moves.
fn horizontal_overlap(
    a: Rectangle<f64, Logical>,
    b: Rectangle<f64, Logical>,
) -> f64 {
    let left = f64::max(a.loc.x, b.loc.x);
    let right = f64::min(a.loc.x + a.size.w, b.loc.x + b.size.w);
    f64::max(0., right - left)
}

#[cfg(test)]
fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/// Spatial direction for navigation and preselection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialDir {
    Up,
    Down,
    Left,
    Right,
}

impl SpatialDir {
    pub fn as_split_side(self) -> SplitSide {
        match self {
            Self::Up => SplitSide::Top,
            Self::Down => SplitSide::Bottom,
            Self::Left => SplitSide::Left,
            Self::Right => SplitSide::Right,
        }
    }
}

/// High-level index-based façade over a [`DwindleTree`] whose leaf values are tile indices.
///
/// This is the "adapter" that scrolling layout code holds. It hides the tree's internal
/// path vocabulary (`LeafPath`, `Child`) behind operations that speak in tile indices, so the
/// scrollable-layout module never touches the recursive tree representation directly. The leaf at
/// position `i` always holds value `i` (the "value == position" invariant), re-established
/// whenever a mutation rebalances the tree.
#[derive(Debug)]
pub struct DwindleColumn {
    tree: DwindleTree<usize>,
}

impl DwindleColumn {
    /// Creates an empty column.
    pub fn new() -> Self {
        Self {
            tree: DwindleTree::new(),
        }
    }

    /// Creates a column with a single (active) leaf holding tile index `0`.
    pub fn single() -> Self {
        Self {
            tree: DwindleTree::single(0),
        }
    }

    /// Returns whether the column has no leaves.
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// Returns the number of leaves (windows) in the column.
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    /// Returns the value (tile index) currently focused by the tree, if any.
    pub fn active_value(&self) -> Option<usize> {
        self.tree.active_value().copied()
    }

    /// Opens a new window (leaf) adjacent to the focused leaf, using the pending preselection (if
    /// any) or the focused leaf's region aspect ratio. The new leaf becomes active.
    ///
    /// The new leaf is temporarily valued `len`, then `reorder` re-establishes value == position.
    /// Returns nothing; callers push the tile first, then call [`Self::reorder`].
    pub fn open_new(&mut self, region: Size<f64, Logical>) {
        self.tree.open_new(self.tree.len(), region);
    }

    /// Like [`Self::open_new`], but forces the split side instead of using any preselection.
    pub fn open_new_on(&mut self, side: SplitSide, region: Size<f64, Logical>) {
        self.tree.open_new_on(self.tree.len(), side, region);
    }

    /// Sets a one-shot directional override for the next [`Self::open_new`].
    pub fn preselect(&mut self, side: SplitSide) {
        self.tree.preselect(side);
    }

    /// Replaces the column's dwindle layout options (split placement, bias, preserve, etc.).
    pub fn set_options(&mut self, options: DwindleOptions) {
        self.tree.set_options(options);
    }

    /// Returns the column's current dwindle layout options.
    pub fn options(&self) -> &DwindleOptions {
        self.tree.options()
    }

    /// Like [`Self::open_new`], but uses `cursor` (in content coordinates) to pick the split
    /// direction when [`DwindleOptions::smart_split`] is enabled.
    pub fn open_new_at(&mut self, region: Size<f64, Logical>, cursor: Point<f64, Logical>) {
        self.tree
            .open_new_at(self.tree.len(), region, cursor);
    }

    /// Removes (expels) the leaf at `tile_idx`, collapsing its vacated container so the sibling
    /// subtree takes over the whole region.
    ///
    /// Returns the removed tile index, or `None` if `tile_idx` was out of range. Afterwards the
    /// remaining leaves are renumbered so value == position.
    pub fn expel_at(&mut self, tile_idx: usize) -> Option<usize> {
        let paths = self.tree.leaf_paths();
        let path = paths.get(tile_idx)?.clone();
        let removed = self.tree.expel(&path)?;
        self.tree.reindex(|value, i| *value = i);
        Some(removed)
    }

    /// Sets the active leaf to the leaf at `tile_idx` (in depth-first order).
    pub fn set_active_at(&mut self, tile_idx: usize) -> bool {
        let paths = self.tree.leaf_paths();
        let Some(path) = paths.get(tile_idx) else {
            return false;
        };
        self.tree.set_active(path)
    }

    /// Returns the leaf values (tile indices) in depth-first order, i.e. the permutation of
    /// `0..len` that maps pre-sort positions to the reordered tile list.
    pub fn dfs_order(&self) -> Vec<usize> {
        self.tree.leaves().copied().collect()
    }

    /// Re-numbers every leaf so that value == position (depth-first order). Use after any tree
    /// mutation to restore the invariant.
    pub fn reindex(&mut self) {
        self.tree.reindex(|value, i| *value = i);
    }

    /// Returns the leaf rectangles partitioning `content`, keyed by leaf value (tile index), in
    /// depth-first order.
    pub fn leaf_rects(
        &self,
        content: Rectangle<f64, Logical>,
        gaps: f64,
    ) -> Vec<(usize, Rectangle<f64, Logical>)> {
        self.tree
            .rects_by_value(content, gaps)
            .into_iter()
            .map(|(v, rect)| (*v, rect))
            .collect()
    }

    /// Returns the rectangle occupied by the leaf at `tile_idx`, if any.
    pub fn leaf_rect(
        &self,
        tile_idx: usize,
        content: Rectangle<f64, Logical>,
        gaps: f64,
    ) -> Option<Rectangle<f64, Logical>> {
        self.leaf_rects(content, gaps)
            .into_iter()
            .find(|(v, _)| *v == tile_idx)
            .map(|(_, rect)| rect)
    }

    /// Finds the tile index spatially adjacent to the leaf at `tile_idx` in `dir`, if any.
    ///
    /// This is the index-based twin of [`DwindleTree::spatial_neighbor`]; it resolves the neighbor
    /// path and reports the tile index that occupies it.
    pub fn spatial_neighbor_idx(
        &self,
        tile_idx: usize,
        dir: SpatialDir,
        content: Rectangle<f64, Logical>,
        gaps: f64,
    ) -> Option<usize> {
        let paths = self.tree.leaf_paths();
        let from = paths.get(tile_idx)?;
        let neighbor = self.tree.spatial_neighbor(from, dir, content, gaps)?;
        paths.iter().position(|p| p == &neighbor)
    }

    /// Returns the tile index whose leaf rectangle contains `pos`, falling back to the leaf whose
    /// center is nearest to `pos` if the point sits in a seam between leaves.
    pub fn leaf_idx_at_point(
        &self,
        pos: Point<f64, Logical>,
        content: Rectangle<f64, Logical>,
        gaps: f64,
    ) -> Option<usize> {
        let rects = self.leaf_rects(content, gaps);
        if let Some((v, _)) = rects.iter().find(|(_, r)| r.contains(pos)) {
            return Some(*v);
        }
        rects
            .iter()
            .map(|(v, r)| {
                let center = Point::<f64, Logical>::from((r.loc.x + r.size.w / 2., r.loc.y + r.size.h / 2.));
                let d = ((pos.x - center.x).powi(2) + (pos.y - center.y).powi(2)).sqrt();
                (d, *v)
            })
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, v)| v)
    }

    /// Adjusts the ratio of the split whose interior edge lies on the `edge` of the leaf at
    /// `tile_idx`, following the pointer by `delta_px`. Returns whether a divider was moved.
    #[allow(clippy::too_many_arguments)]
    pub fn adjust_ratio_for_edge(
        &mut self,
        tile_idx: usize,
        edge: ResizeEdge,
        delta_px: f64,
        usable_w: f64,
        usable_h: f64,
        gaps: f64,
        min_w: &impl Fn(&usize) -> f64,
        min_h: &impl Fn(&usize) -> f64,
    ) -> bool {
        let paths = self.tree.leaf_paths();
        let Some(path) = paths.get(tile_idx) else {
            return false;
        };
        self.tree.adjust_ratio_for_edge(
            path, edge, delta_px, usable_w, usable_h, gaps, min_w, min_h,
        )
    }

    /// Flips the split orientation of the container directly holding the leaf at `tile_idx`.
    /// Returns whether a split was flipped.
    pub fn toggle_split_at(&mut self, tile_idx: usize) -> bool {
        let paths = self.tree.leaf_paths();
        let Some(path) = paths.get(tile_idx) else {
            return false;
        };
        self.tree.toggle_split(path)
    }

    /// Moves the leaf (window) at `tile_idx` to the head of the tree, keeping focus on it. Returns
    /// whether a move took place. Afterwards the leaves are renumbered so value == position.
    pub fn promote_at(&mut self, tile_idx: usize) -> bool {
        let paths = self.tree.leaf_paths();
        let Some(path) = paths.get(tile_idx) else {
            return false;
        };
        if self.tree.promote(path) {
            // Keep the focus on the moved window, which now sits in the first leaf.
            if let Some(head) = self.tree.first_leaf_path() {
                self.tree.set_active(&head);
            }
            self.tree.reindex(|value, i| *value = i);
            true
        } else {
            false
        }
    }
}

impl Default for DwindleColumn {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn square() -> Size<f64, Logical> {
        Size::from((100., 100.))
    }

    fn wide() -> Size<f64, Logical> {
        Size::from((100., 50.))
    }

    fn tall() -> Size<f64, Logical> {
        Size::from((50., 100.))
    }

    fn build_chain(count: usize) -> DwindleTree<i32> {
        let mut tree = DwindleTree::new();
        for i in 0..count {
            tree.open_new_on(i as i32, SplitSide::Bottom, square());
        }
        tree
    }

    fn walk<'a>(tree: &'a DwindleTree<i32>, path: &[Child]) -> &'a Node<i32> {
        let mut node = tree.root.as_ref().unwrap();
        for c in path {
            let Node::Split { first, second, .. } = node else {
                panic!("expected split at path {path:?}");
            };
            node = match c {
                Child::First => first,
                Child::Second => second,
            };
        }
        node
    }

    fn assert_axis(tree: &DwindleTree<i32>, leaf_path: &[Child], expected: SplitAxis) {
        let container = &leaf_path[..leaf_path.len().saturating_sub(1)];
        let Node::Split { axis, .. } = walk(tree, container) else {
            panic!("leaf {leaf_path:?} has no container split");
        };
        assert_eq!(*axis, expected);
    }

    #[test]
    fn opens_stack_with_bottom_splits() {
        let tree = build_chain(4);
        assert_eq!(tree.len(), 4);
        assert_eq!(tree.leaves().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        assert_eq!(
            tree.leaf_paths(),
            vec![
                LeafPath(vec![Child::First]),
                LeafPath(vec![Child::Second, Child::First]),
                LeafPath(vec![Child::Second, Child::Second, Child::First]),
                LeafPath(vec![Child::Second, Child::Second, Child::Second]),
            ]
        );
        // Forced-bottom splits keep stacking: every container split is horizontal.
        assert_axis(&tree, &[Child::First], SplitAxis::Horizontal);
        assert_axis(&tree, &[Child::Second, Child::First], SplitAxis::Horizontal);
        assert_axis(&tree, &[Child::Second, Child::Second, Child::First], SplitAxis::Horizontal);
    }

    #[test]
    fn aspect_based_splitting_mixes_axes() {
        // Using aspect-based default sides (Hyprland's `split_width_multiplier` semantics), the
        // active leaf's region drives the split orientation: wide/square regions split
        // side-by-side, tall regions stack. The new window always takes the right/bottom half, so
        // DFS order is insertion order: open_new on square regions produces V{0, H{1, V{2, 3}}}.
        let mut tree = DwindleTree::new();
        tree.open_new(0, square());
        tree.open_new(1, square());
        tree.open_new(2, square());
        tree.open_new(3, square());
        assert_eq!(tree.leaves().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        assert_axis(&tree, &[Child::First], SplitAxis::Vertical);
        assert_axis(&tree, &[Child::Second, Child::First], SplitAxis::Horizontal);
        assert_axis(&tree, &[Child::Second, Child::Second, Child::First], SplitAxis::Vertical);
    }

    #[test]
    fn new_window_becomes_active() {
        let mut tree = DwindleTree::new();
        let r = tree.open_new(1, square());
        assert!(r.is_empty());
        assert_eq!(tree.active().unwrap(), &r);

        let r2 = tree.open_new(2, square());
        assert_eq!(tree.active_value(), Some(&2));
        assert_eq!(r2, LeafPath(vec![Child::Second]));
    }

    #[test]
    fn wide_region_splits_side_by_side() {
        let mut tree = DwindleTree::single(1);
        tree.open_new(2, wide());
        assert_axis(&tree, &[Child::First], SplitAxis::Vertical);
        // The focused window keeps the first (left) child, the new window takes the right half.
        assert_eq!(tree.leaf(&LeafPath(vec![Child::First])), Some(&1));
        assert_eq!(tree.leaf(&LeafPath(vec![Child::Second])), Some(&2));
    }

    #[test]
    fn tall_region_splits_horizontally() {
        let mut tree = DwindleTree::single(1);
        tree.open_new(2, tall());
        assert_axis(&tree, &[Child::First], SplitAxis::Horizontal);
    }

    #[test]
    fn preselect_overrides_default_side() {
        let mut tree = DwindleTree::single(1);
        tree.preselect(SplitSide::Left);
        let path = tree.open_new(2, square());
        assert_eq!(path, LeafPath(vec![Child::First]));
        assert_axis(&tree, &[Child::First], SplitAxis::Vertical);
        assert_eq!(tree.pending_preselection(), None);

        tree.preselect(SplitSide::Right);
        tree.open_new(3, square());
        // Tree: V{ V{3, 2}, 1}
        assert_eq!(
            tree.leaf_paths(),
            vec![
                LeafPath(vec![Child::First, Child::First]),
                LeafPath(vec![Child::First, Child::Second]),
                LeafPath(vec![Child::Second]),
            ]
        );

        tree.preselect(SplitSide::Top);
        tree.open_new(4, square());
        tree.preselect(SplitSide::Bottom);
        tree.open_new(5, square());
        assert_eq!(tree.active_value(), Some(&5));
    }

    #[test]
    fn preselect_is_one_shot() {
        let mut tree = DwindleTree::single(1);
        tree.preselect(SplitSide::Left);
        tree.open_new(2, square());
        assert!(tree.pending_preselection().is_none());
        tree.open_new(3, square());
        // Second open used the aspect-based default, not the consumed Left.
        assert_axis(&tree, &[Child::First], SplitAxis::Vertical);
    }

    #[test]
    fn toggle_split_flips_container() {
        let mut tree = build_chain(2);
        assert_axis(&tree, &[Child::First], SplitAxis::Horizontal);
        let leaf = LeafPath(vec![Child::Second]);
        assert!(tree.toggle_split(&leaf));
        assert_axis(&tree, &[Child::First], SplitAxis::Vertical);
        // Values are unchanged; only orientation flipped.
        assert_eq!(tree.leaves().copied().collect::<Vec<_>>(), vec![0, 1]);
        assert!(tree.toggle_split(&leaf));
        assert_axis(&tree, &[Child::First], SplitAxis::Horizontal);
    }

    #[test]
    fn expel_removes_leaf_and_collapses() {
        let mut tree = build_chain(4);
        let out = tree.expel(&LeafPath(vec![
            Child::Second,
            Child::Second,
            Child::First,
        ]));
        assert_eq!(out, Some(2));
        assert_eq!(tree.leaves().copied().collect::<Vec<_>>(), vec![0, 1, 3]);
        // The vacated container collapsed; the sibling of the expelled leaf is 3.
        assert_eq!(
            tree.leaf(&LeafPath(vec![Child::Second, Child::Second])),
            Some(&3)
        );
    }

    #[test]
    fn expel_restores_on_bad_path() {
        let mut tree = DwindleTree::single(10);
        // Invalid path on a single-leaf tree is a no-op.
        assert_eq!(tree.expel(&LeafPath(vec![Child::First])), None);
        assert_eq!(tree.len(), 1);
        // The root path removes the only leaf.
        assert_eq!(tree.expel(&LeafPath::root()), Some(10));
        assert!(tree.is_empty());
        assert_eq!(tree.active(), None);
        // A split-root tree cannot be expelled by its root path.
        let mut tree = build_chain(2);
        assert_eq!(tree.expel(&LeafPath::root()), None);
        assert_eq!(tree.expel(&LeafPath(vec![Child::First])), Some(0));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn expel_active_leaf_focuses_neighbor() {
        let mut tree = build_chain(3);
        tree.open_new(3, square()); // active leaf is the last one (3)
        assert_eq!(tree.active_value(), Some(&3));
        let active = tree.active().unwrap().clone();
        tree.expel(&active);
        assert_eq!(tree.len(), 3);
        assert!(tree.active_value().is_some());
    }

    #[test]
    fn consume_absorbs_sibling_subtree() {
        let mut tree = build_chain(4);
        // Focused leaf at [Second, Second, First] = 2; its sibling subtree is
        // [Second, Second, Second] containing just 3.
        let consumed = tree.consume(&LeafPath(vec![
            Child::Second,
            Child::Second,
            Child::First,
        ]));
        assert_eq!(consumed, Some(vec![3]));
        assert_eq!(tree.leaves().copied().collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(tree.active_value(), Some(&2));
    }

    #[test]
    fn consume_absorbs_big_sibling() {
        let mut tree = build_chain(5);
        // Focused leaf 0 at [First]. Its sibling subtree [Second] holds 1,2,3,4.
        let consumed = tree.consume(&LeafPath(vec![Child::First]));
        assert_eq!(consumed, Some(vec![1, 2, 3, 4]));
        assert_eq!(tree.leaves().copied().collect::<Vec<_>>(), vec![0]);
        assert_eq!(tree.active_value(), Some(&0));
    }

    #[test]
    fn consume_single_leaf_is_noop() {
        let mut tree = DwindleTree::single(1);
        assert_eq!(tree.consume(&LeafPath::root()), None);
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn promote_moves_value_to_head() {
        let mut tree = build_chain(4);
        // Value 2 lives at [Second, Second, First] in the forced-bottom chain.
        assert!(tree.promote(&LeafPath(vec![
            Child::Second,
            Child::Second,
            Child::First,
        ])));
        // Value 2 takes the head slot; the old head value 0 lands where 2 was.
        assert_eq!(tree.leaves().copied().collect::<Vec<_>>(), vec![2, 1, 0, 3]);
    }

    #[test]
    fn swap_leaves_exchanges_values() {
        let mut tree = build_chain(3);
        tree.swap_leaves(
            &LeafPath(vec![Child::Second, Child::First]),
            &LeafPath(vec![Child::Second, Child::Second]),
        );
        assert_eq!(tree.leaves().copied().collect::<Vec<_>>(), vec![0, 2, 1]);
    }

    #[test]
    fn focus_by_walks_tree_order() {
        let mut tree = build_chain(4);
        let p0 = LeafPath(vec![Child::First]);
        assert_eq!(tree.focus_by(&p0, 1), Some(LeafPath(vec![Child::Second, Child::First])));
        assert_eq!(tree.focus_by(&p0, -1), Some(p0.clone()));
        assert_eq!(
            tree.focus_by(&p0, 100),
            Some(LeafPath(vec![Child::Second, Child::Second, Child::Second]))
        );
    }

    #[test]
    fn adjust_ratio_respects_bounds() {
        let mut tree = build_chain(2);
        let leaf = LeafPath(vec![Child::Second]);
        assert!(tree.adjust_ratio(&leaf, -10.));
        let Node::Split { ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, MIN_RATIO);
        assert!(tree.adjust_ratio(&leaf, 10.));
        let Node::Split { ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, MAX_RATIO);
    }

    #[test]
    fn adjust_ancestor_ratio_moves_nearest_matching_split() {
        // Tree with a vertical split nested inside a horizontal one:
        // H{ A, V{ B, C } }  (A on top, B left, C right)
        let mut tree = DwindleTree::new();
        tree.open_new(0, square());
        tree.open_new_on(1, SplitSide::Bottom, square());
        tree.open_new_on(2, SplitSide::Right, wide());
        // A = [First], B = [Second, First], C = [Second, Second]

        // Resizing B (left/First child of the vertical split) grows it rightward: the vertical
        // split ratio (First share) increases with a positive drag.
        let b = LeafPath(vec![Child::Second, Child::First]);
        assert_eq!(tree.leaf_side_in_split(&b, SplitAxis::Vertical), Some(Child::First));
        assert!(tree.adjust_ancestor_ratio(&b, SplitAxis::Vertical, 50., 200.));
        let Node::Split { axis: SplitAxis::Vertical, ratio, .. } = walk(
            &tree,
            &[Child::Second],
        ) else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO + 0.5);

        // A horizontal drag on B must NOT touch the vertical split (no matching divider to move
        // there), so the vertical ratio is unchanged and the outer horizontal split is adjusted.
        assert!(tree.adjust_ancestor_ratio(&b, SplitAxis::Horizontal, 20., 200.));
        let Node::Split { axis: SplitAxis::Horizontal, ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO + 0.2);
        let Node::Split { axis: SplitAxis::Vertical, ratio, .. } = walk(
            &tree,
            &[Child::Second],
        ) else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO + 0.5);

        // Resizing C (right/Second child) with the same positive drag also moves the same divider
        // rightward, further growing B (First), so the ratio increases again.
        let c = LeafPath(vec![Child::Second, Child::Second]);
        assert_eq!(tree.leaf_side_in_split(&c, SplitAxis::Vertical), Some(Child::Second));
        assert!(tree.adjust_ancestor_ratio(&c, SplitAxis::Vertical, 10., 100.));
        let Node::Split { axis: SplitAxis::Vertical, ratio, .. } = walk(
            &tree,
            &[Child::Second],
        ) else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO + 0.5 + 0.2);
    }

    #[test]
    fn adjust_ancestor_ratio_clamps_to_bounds() {
        let mut tree = build_chain(2);
        let leaf = LeafPath(vec![Child::Second]);
        assert!(tree.adjust_ancestor_ratio(&leaf, SplitAxis::Horizontal, -10000., 100.));
        let Node::Split { ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, MIN_RATIO);
        assert!(tree.adjust_ancestor_ratio(&leaf, SplitAxis::Horizontal, 10000., 100.));
        let Node::Split { ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, MAX_RATIO);
    }

    #[test]
    fn adjust_ancestor_ratio_noop_when_no_matching_axis() {
        let mut tree = build_chain(2); // only a horizontal split
        let leaf = LeafPath(vec![Child::Second]);
        assert_eq!(tree.leaf_side_in_split(&leaf, SplitAxis::Vertical), None);
        assert!(!tree.adjust_ancestor_ratio(&leaf, SplitAxis::Vertical, 10., 100.));
    }

    #[test]
    fn adjust_ratio_for_edge_scales_nested_divider_by_own_box() {
        // V_root{ V_inner{0, 2}, 1 }: leaf 2 shares a vertical divider with 0 inside a vertical
        // split that only spans the root's left half (100 wide in a 200-wide root). Dragging that
        // divider must scale by the inner split's OWN 100px width (10px -> ratio +0.1), not the
        // whole column's 200px (which would be +0.05) -- that was the "both windows resize
        // weirdly" bug for nested side-by-side splits.
        let mut tree = DwindleTree::new();
        tree.open_new(0, square());               // leaf 0
        tree.open_new_on(1, SplitSide::Right, square()); // V{0, 1}, active 1
        tree.set_active(&LeafPath(vec![Child::First])); // active 0
        tree.open_new_on(2, SplitSide::Right, square()); // V{ V{0, 2}, 1 }, active 2
        let two = LeafPath(vec![Child::First, Child::Second]);
        let no_min = |_v: &i32| 1.;

        // Inner split is 100 wide, so +10px moves its divider by 0.2 of ratio (its share is
        // ratio/2, so the pixel-to-ratio factor is 2/usable = 2/100).
        assert!(tree.adjust_ratio_for_edge(&two, ResizeEdge::LEFT, 10., 200., 200., 0., &no_min, &no_min));
        let Node::Split {
            axis: SplitAxis::Vertical,
            ratio,
            ..
        } = walk(&tree, &[Child::First])
        else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO + 0.2);

        // -20px back down the 100px scale returns it below the default.
        assert!(tree.adjust_ratio_for_edge(&two, ResizeEdge::LEFT, -20., 200., 200., 0., &no_min, &no_min));
        let Node::Split {
            axis: SplitAxis::Vertical,
            ratio,
            ..
        } = walk(&tree, &[Child::First])
        else {
            unreachable!();
        };
        assert!(approx_eq(*ratio, DEFAULT_RATIO - 0.2));

        // The root divider (200 wide) was not touched.
        let Node::Split {
            axis: SplitAxis::Vertical,
            ratio,
            ..
        } = walk(&tree, &[])
        else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO);
    }

    #[test]
    fn adjust_ratio_for_edge_moves_outer_shared_divider() {
        // V_root{ V_inner{0, 2}, 1 }: leaf 2 sits at [First, Second]. Its right edge is the ROOT
        // divider — the old nearest-matching-split logic could not move that edge, leaving the
        // divider dead even though it is a real shared border.
        let mut tree = DwindleTree::new();
        tree.open_new(0, square());               // leaf 0
        tree.open_new_on(1, SplitSide::Right, square()); // V{0, 1}, active 1
        tree.set_active(&LeafPath(vec![Child::First])); // active 0
        tree.open_new_on(2, SplitSide::Right, square()); // V{ V{0, 2}, 1 }, active 2
        let deep = LeafPath(vec![Child::First, Child::Second]);
        let no_min = |_v: &i32| 1.;

        assert!(tree.adjust_ratio_for_edge(&deep, ResizeEdge::RIGHT, 50., 200., 200., 0., &no_min, &no_min));
        let Node::Split { ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO + 0.5);

        // The leaf's other (interior) edge still targets the deeper split, which stayed put.
        let Node::Split { ratio, .. } = walk(&tree, &[Child::First]) else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO);
    }

    #[test]
    fn adjust_ratio_for_edge_refuses_exterior_edges() {
        let mut tree = build_chain(2); // H{0, 1}
        let no_min = |_v: &i32| 1.;

        // Leaf 0's top edge is the content boundary, not a divider.
        let leaf = LeafPath(vec![Child::First]);
        assert!(!tree.adjust_ratio_for_edge(&leaf, ResizeEdge::TOP, 10., 200., 100., 0., &no_min, &no_min));
        // Leaf 0's bottom edge is the shared divider; it moves.
        assert!(tree.adjust_ratio_for_edge(&leaf, ResizeEdge::BOTTOM, 10., 200., 100., 0., &no_min, &no_min));

        // Leaf 1's top edge is the same divider; its bottom edge is the content boundary.
        let leaf = LeafPath(vec![Child::Second]);
        assert!(!tree.adjust_ratio_for_edge(&leaf, ResizeEdge::BOTTOM, 10., 200., 100., 0., &no_min, &no_min));
        assert!(tree.adjust_ratio_for_edge(&leaf, ResizeEdge::TOP, 10., 200., 100., 0., &no_min, &no_min));
    }

    #[test]
    fn adjust_ratio_for_edge_clamps_to_subtree_minimums() {
        // H{0, 1} in a 100-tall usable space; both leaves need at least 30 in height.
        let mut tree = build_chain(2);
        let leaf = LeafPath(vec![Child::Second]);
        let no_min_w = |_v: &i32| 1.;
        let min_h = |v: &i32| if *v == 0 || *v == 1 { 30. } else { 0. };

        // A huge upward drag (growing the Second child) stops at the First child's minimum.
        assert!(tree.adjust_ratio_for_edge(&leaf, ResizeEdge::TOP, -1000., 200., 100., 0., &no_min_w, &min_h));
        let Node::Split { ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, 0.6);

        // A huge downward drag (growing the First child) stops at the Second child's minimum.
        assert!(tree.adjust_ratio_for_edge(&leaf, ResizeEdge::TOP, 1000., 200., 100., 0., &no_min_w, &min_h));
        let Node::Split { ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, 1.4);
    }

    #[test]
    fn adjust_ratio_for_edge_noop_when_minimums_do_not_fit() {
        // The leaves' minimums (60 each) exceed the usable height (100): the divider is pinned.
        let mut tree = build_chain(2);
        let leaf = LeafPath(vec![Child::Second]);
        let no_min_w = |_v: &i32| 1.;
        let min_h = |_v: &i32| 60.;
        assert!(!tree.adjust_ratio_for_edge(&leaf, ResizeEdge::TOP, 10., 200., 100., 0., &no_min_w, &min_h));
        let Node::Split { ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO);
    }

    #[test]
    fn leaf_rects_share_divider_on_whole_pixels() {
        // Odd content width with a fractional divider: the First child floors to a whole logical
        // pixel and the Second child takes the remainder, so the shared divider is exactly the
        // same integer boundary for both leaves and the partition still tiles the content.
        let mut tree = DwindleTree::new();
        tree.open_new(0, wide());
        tree.open_new_on(1, SplitSide::Right, wide()); // V{0, 1}
        let no_min = |_v: &i32| 1.;
        // Push the divider to a fractionally-lying ratio: 0.5 + 3.7/127.
        assert!(tree.adjust_ratio_for_edge(
            &LeafPath(vec![Child::First]),
            ResizeEdge::RIGHT,
            3.7,
            127.,
            100.,
            0.,
            &no_min,
            &no_min,
        ));

        let content = Rectangle::new(Point::from((0., 0.)), Size::from((127., 100.)));
        let rects = tree.leaf_rects(content, 0.);
        assert_eq!(rects.len(), 2);
        let (first, second) = (rects[0].1, rects[1].1);
        assert_eq!(
            first.size.w,
            first.size.w.floor(),
            "first child width must be whole pixels: {first:?}"
        );
        assert_eq!(
            first.loc.x + first.size.w,
            second.loc.x,
            "the shared divider must be one boundary: {first:?} vs {second:?}"
        );
        assert_eq!(first.size.w + second.size.w, content.size.w);
    }

    #[test]
    fn leaf_rects_stack_vertically() {
        let tree = build_chain(3);
        let content = Rectangle::new(Point::from((0., 0.)), Size::from((200., 300.)));
        let rects = tree.leaf_rects(content, 0.);
        assert_eq!(rects.len(), 3);
        // Pure vertical stack: all leaves are full-width; heights halve each level: 150/75/75.
        for (_, rect) in &rects {
            assert_eq!(rect.size.w, 200.);
        }
        assert_eq!(rects[0].1.size.h, 150.);
        assert_eq!(rects[1].1.size.h, 75.);
        assert_eq!(rects[2].1.size.h, 75.);
        for pair in rects.windows(2) {
            let above = &pair[0].1;
            let below = &pair[1].1;
            assert!(above.loc.y < below.loc.y);
            assert!(below.loc.y >= above.loc.y + above.size.h);
        }
    }

    #[test]
    fn leaf_rects_insert_seams() {
        let tree = build_chain(2);
        let content = Rectangle::new(Point::from((0., 0.)), Size::from((200., 90.)));
        let gaps = 10.;
        let rects = tree.leaf_rects(content, gaps);
        assert_eq!(rects.len(), 2);
        let total_h: f64 = rects.iter().map(|(_, r)| r.size.h).sum();
        assert_eq!(total_h + gaps, content.size.h);
    }

    #[test]
    fn spatial_neighbor_navigates_directions() {
        // Tree: H{0, V{1, 2}} over (0,0,1000x1000):
        //   leaf 0 = (0,0,1000x500) full-width top,
        //   leaf 1 = (0,500,500x500) bottom-left, leaf 2 = (500,500,500x500) bottom-right.
        let mut tree = DwindleTree::single(0);
        tree.open_new_on(1, SplitSide::Bottom, square());
        tree.open_new_on(2, SplitSide::Right, wide());
        let content = Rectangle::new(Point::from((0., 0.)), Size::from((1000., 1000.)));
        let gaps = 0.;

        let leaf0 = LeafPath(vec![Child::First]); // value 0, full-width top
        let leaf1 = LeafPath(vec![Child::Second, Child::First]); // value 1, bottom-left
        let leaf2 = LeafPath(vec![Child::Second, Child::Second]); // value 2, bottom-right

        // Navigation follows real dividers: the window behind the far edge in `dir` that overlaps
        // the source along the perpendicular axis; on a tie the DFS-first candidate wins.
        assert_eq!(
            tree.spatial_neighbor(&leaf2, SpatialDir::Left, content, gaps),
            Some(leaf1.clone()),
            "bottom-right moves left across its own divider to bottom-left"
        );
        assert_eq!(
            tree.spatial_neighbor(&leaf1, SpatialDir::Up, content, gaps),
            Some(leaf0.clone()),
            "bottom-left moves up across the root divider to the top leaf"
        );
        assert_eq!(
            tree.spatial_neighbor(&leaf0, SpatialDir::Down, content, gaps),
            Some(leaf1.clone()),
            "the whole top-leaf bottom edge is the root divider; the DFS-first bottom leaf wins the tie"
        );
        assert_eq!(
            tree.spatial_neighbor(&leaf2, SpatialDir::Right, content, gaps),
            None,
            "the bottom-right leaf has no divider to its right"
        );
        assert_eq!(
            tree.spatial_neighbor(&leaf0, SpatialDir::Right, content, gaps),
            None,
            "the full-width top leaf has no rightward divider (no diagonal hop to the bottom-right)"
        );
        assert_eq!(
            tree.spatial_neighbor(&leaf1, SpatialDir::Left, content, gaps),
            None,
            "the bottom-left leaf has no divider to its left"
        );
    }

    fn rects_overlap(a: Rectangle<f64, Logical>, b: Rectangle<f64, Logical>) -> bool {
        a.loc.x < b.loc.x + b.size.w
            && b.loc.x < a.loc.x + a.size.w
            && a.loc.y < b.loc.y + b.size.h
            && b.loc.y < a.loc.y + a.size.h
    }

    fn assert_valid_partition(tree: &DwindleTree<i32>, content: Rectangle<f64, Logical>, gaps: f64) {
        let rects = tree.leaf_rects(content, gaps);
        assert_eq!(rects.len(), tree.len());

        // Counts and values are consistent.
        let values = tree.leaves().copied().collect::<Vec<_>>();
        assert_eq!(values.len(), tree.len());
        let unique: std::collections::HashSet<i32> = values.iter().copied().collect();
        assert_eq!(unique.len(), values.len(), "leaf values must stay unique");

        for (path, rect) in &rects {
            assert!(tree.leaf(path).is_some(), "rect path must resolve to a leaf");
            assert!(rect.size.w >= 0. && rect.size.h >= 0.);
            assert!(
                rect.loc.x >= -1e-6
                    && rect.loc.y >= -1e-6
                    && rect.loc.x + rect.size.w <= content.size.w + 1e-6
                    && rect.loc.y + rect.size.h <= content.size.h + 1e-6,
                "rect must be inside content: {rect:?}"
            );
        }

        for (i, (_, a)) in rects.iter().enumerate() {
            for (_, b) in rects.iter().skip(i + 1) {
                assert!(
                    !rects_overlap(*a, *b),
                    "leaf rects must not overlap: {a:?} vs {b:?}"
                );
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum Op {
        New(f64, f64),
        Toggle,
        Swap,
        Expel,
        Consume,
    }

    fn op_strategy() -> impl Strategy<Value = Op> {
        prop_oneof![
            (50..2000_i32, 50..2000_i32).prop_map(|(w, h)| Op::New(w as f64, h as f64)),
            Just(Op::Toggle),
            Just(Op::Swap),
            Just(Op::Expel),
            Just(Op::Consume),
        ]
    }

    fn deterministic_index(seed: usize, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        seed.wrapping_mul(2654435761) % len
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        #[test]
        fn fuzz_partition_invariants(
            ops in prop::collection::vec(op_strategy(), 1..60)
        ) {
            let content = Rectangle::new(Point::from((0., 0.)), Size::from((1847., 1023.)));
            let mut tree = DwindleTree::new();
            let mut next_value = 0i32;

            for op in ops {
                match op {
                    Op::New(w, h) => {
                        let region = Size::from((w, h));
                        tree.open_new(next_value, region);
                        next_value += 1;
                    }
                    Op::Toggle => {
                        let paths = tree.leaf_paths();
                        let idx = deterministic_index(next_value as usize, paths.len());
                        if let Some(p) = paths.into_iter().nth(idx) {
                            tree.toggle_split(&p);
                        }
                    }
                    Op::Swap => {
                        let paths = tree.leaf_paths();
                        if paths.len() >= 2 {
                            let i = deterministic_index(next_value as usize, paths.len());
                            let j = deterministic_index(next_value as usize + 1, paths.len());
                            tree.swap_leaves(&paths[i], &paths[j]);
                        }
                    }
                    Op::Expel => {
                        let paths = tree.leaf_paths();
                        let idx = deterministic_index(next_value as usize, paths.len());
                        if let Some(p) = paths.into_iter().nth(idx) {
                            tree.expel(&p);
                        }
                    }
                    Op::Consume => {
                        let paths = tree.leaf_paths();
                        let idx = deterministic_index(next_value as usize, paths.len());
                        if let Some(p) = paths.into_iter().nth(idx) {
                            tree.consume(&p);
                        }
                    }
                }

                assert_valid_partition(&tree, content, 4.);
                assert_valid_partition(&tree, content, 0.);
            }
        }

        #[test]
        fn fuzz_expel_removes_one_leaf(
            ops in prop::collection::vec(op_strategy(), 1..60)
        ) {
            let mut tree = DwindleTree::new();
            let mut next_value = 0i32;
            for op in ops {
                match op {
                    Op::New(w, h) => {
                        let region = Size::from((w, h));
                        tree.open_new(next_value, region);
                        next_value += 1;
                    }
                    Op::Expel => {
                        let paths = tree.leaf_paths();
                        let before = tree.len();
                        let idx = deterministic_index(next_value as usize, paths.len());
                        if let Some(p) = paths.into_iter().nth(idx) {
                            let expelled = tree.expel(&p);
                            assert_eq!(tree.len(), before - 1);
                            assert!(expelled.is_some());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

        #[test]
        fn force_split_first_places_new_window_in_first_half() {
            let mut tree = DwindleTree::single(1);
            tree.set_options(DwindleOptions {
                force_split: ForceSplit::First,
                ..DwindleOptions::default()
            });
            tree.open_new(2, wide());
            // Wide region would normally place the new window in the Right (Second) half, but
            // ForceSplit::First overrides placement to the Left (First) half.
            assert_eq!(tree.leaf(&LeafPath(vec![Child::First])), Some(&2));
            assert_eq!(tree.leaf(&LeafPath(vec![Child::Second])), Some(&1));
        }

        #[test]
        fn force_split_second_places_new_window_in_second_half() {
            let mut tree = DwindleTree::single(1);
            tree.set_options(DwindleOptions {
                force_split: ForceSplit::Second,
                ..DwindleOptions::default()
            });
            tree.open_new(2, wide());
            assert_eq!(tree.leaf(&LeafPath(vec![Child::First])), Some(&1));
            assert_eq!(tree.leaf(&LeafPath(vec![Child::Second])), Some(&2));
        }

        #[test]
        fn split_bias_flips_default_ratio_for_first_child() {
            // With split_bias on and a default ratio favouring Second (1.5), a window placed in the
            // First half must be given the flipped ratio (2 - 1.5 = 0.5) so it gets the bigger share.
            let mut tree = DwindleTree::single(1);
            tree.set_options(DwindleOptions {
                split_bias: true,
                default_split_ratio: 1.5,
                ..DwindleOptions::default()
            });
            tree.preselect(SplitSide::Left);
            tree.open_new(2, square());
            let Node::Split { ratio, .. } = walk(&tree, &[]) else {
                panic!("expected a split");
            };
            assert_eq!(*ratio, 0.5);

            // Without split_bias the default ratio is left untouched for a First placement.
            let mut plain = DwindleTree::single(1);
            plain.set_options(DwindleOptions {
                split_bias: false,
                default_split_ratio: 1.5,
                ..DwindleOptions::default()
            });
            plain.preselect(SplitSide::Left);
            plain.open_new(2, square());
            let Node::Split { ratio, .. } = walk(&plain, &[]) else {
                panic!("expected a split");
            };
            assert_eq!(*ratio, 1.5);
        }

        #[test]
        fn smart_split_side_follows_cursor_around_center() {
            let wide = Rectangle::new(Point::from((0., 0.)), Size::from((100., 50.)));
            // Right half: |dx| small relative to the wide box.
            assert_eq!(
                smart_split_side(wide, Point::from((80., 25.))),
                SplitSide::Right
            );
            assert_eq!(
                smart_split_side(wide, Point::from((20., 25.))),
                SplitSide::Left
            );
            // Center column: tall enough to switch to top/bottom.
            assert_eq!(
                smart_split_side(wide, Point::from((50., 40.))),
                SplitSide::Bottom
            );
            assert_eq!(
                smart_split_side(wide, Point::from((50., 10.))),
                SplitSide::Top
            );
            // Exactly at the center has no well-defined half; falls back to the aspect-ratio
            // default for a wide region: right-side placement.
            assert_eq!(
                smart_split_side(wide, Point::from((50., 25.))),
                SplitSide::Right
            );
        }

        #[test]
        fn permanent_direction_override_keeps_preselection() {
            let mut tree = DwindleTree::single(1);
            tree.set_options(DwindleOptions {
                permanent_direction_override: true,
                ..DwindleOptions::default()
            });
            tree.preselect(SplitSide::Left);
            tree.open_new(2, square());
            // The preselection is NOT consumed while permanent_direction_override is set.
            assert_eq!(tree.pending_preselection(), Some(SplitSide::Left));
            tree.open_new(3, square());
            // Still placing windows in the First (left) half.
            assert_eq!(tree.leaf(&LeafPath(vec![Child::First, Child::First])), Some(&3));
            assert_eq!(tree.pending_preselection(), Some(SplitSide::Left));
        }
}