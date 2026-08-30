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

    #[cfg(test)]
    fn child(&self, idx: usize) -> Child {
        self.0[idx]
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

/// Default visual ratio of a freshly created split's `First` child.
pub const DEFAULT_RATIO: f64 = 0.5;

/// Minimum ratio enforced when adjusting a split's ratio interactively.
pub const MIN_RATIO: f64 = 0.1;

/// Maximum ratio enforced when adjusting a split's ratio interactively.
pub const MAX_RATIO: f64 = 1. - MIN_RATIO;

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
}

impl<T> DwindleTree<T> {
    /// Creates an empty tree.
    pub fn new() -> Self {
        Self {
            root: None,
            active: None,
            preselect: None,
        }
    }

    /// Creates a tree with a single leaf, which is also the active leaf.
    pub fn single(value: T) -> Self {
        Self {
            root: Some(Node::Leaf(value)),
            active: Some(LeafPath::root()),
            preselect: None,
        }
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
            (Node::Split { first, second: _, .. }, Some(Child::First)) => {
                self.leaf_impl(first, &path[1..])
            }
            (Node::Split { first: _, second, .. }, Some(Child::Second)) => {
                self.leaf_impl(second, &path[1..])
            }
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
    /// The active leaf's region is split per the pending preselection (if any, consumed) or else
    /// per its current width-to-height ratio. The new leaf becomes the active leaf.
    ///
    /// Returns the path of the newly opened leaf.
    pub fn open_new(&mut self, value: T, region: Size<f64, Logical>) -> LeafPath {
        let side_override = self.preselect.take();
        self.open_new_inner(value, side_override, region)
    }

    /// Like [`Self::open_new`], with an explicit side override instead of a preset.
    pub fn open_new_on(&mut self, value: T, side: SplitSide, region: Size<f64, Logical>) -> LeafPath {
        self.open_new_inner(value, Some(side), region)
    }

    fn open_new_inner(
        &mut self,
        value: T,
        side_override: Option<SplitSide>,
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
                default_side_for_aspect(rect.size)
            }
        };

        let root = self.root.take().unwrap();
        let (new_root, new_path) = open_leaf_at(root, active.0.as_slice(), side, value);
        self.root = Some(new_root);
        self.active = Some(new_path.clone());
        new_path
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
                let active_was_removed = self.active.as_ref().map_or(true, |a| a == path);
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
        let idx = match paths.iter().position(|p| p == from) {
            Some(idx) => idx,
            None => return None,
        };
        let len = paths.len() as i32;
        let new = (idx as i32 + step).clamp(0, len - 1);
        let path = paths[new as usize].clone();
        self.active = Some(path.clone());
        Some(path)
    }

    /// Finds the spatial neighbor of `from` in the given direction, using the leaf rectangles
    /// returned by [`Self::leaf_rects`] over `content`.
    ///
    /// Returns the closest leaf rectangle in that direction. `None` when there is no leaf in that
    /// direction.
    pub fn spatial_neighbor(
        &self,
        from: &LeafPath,
        dir: SpatialDir,
        content: Rectangle<f64, Logical>,
        gaps: f64,
    ) -> Option<LeafPath> {
        let rects = self.leaf_rects(content, gaps);
        let from_rect = rects.iter().find(|(p, _)| p == from)?.1;
        let from_center = center(from_rect);

        let mut best: Option<(f64, f64, &LeafPath)> = None;
        for (path, rect) in &rects {
            if path == from {
                continue;
            }
            let (dist, overlap) =
                match directional_score(from_center, center(*rect), from_rect, *rect, dir) {
                    Some(score) => score,
                    None => continue,
                };
            let better = match best {
                None => true,
                Some((best_dist, best_overlap, _)) => {
                    dist < best_dist || (approx_eq(dist, best_dist) && overlap > best_overlap)
                }
            };
            if better {
                best = Some((dist, overlap, path));
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
        adjust_ancestor_ratio_impl(root, &path.0, axis, delta_px / usable)
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
            // Clamp so a region smaller than the seam can't produce a negative split size.
            let usable = (rect.size.h - gaps).max(0.);
            let first_h = (usable * ratio).clamp(0., usable);
            let second_h = usable - first_h;
            let first = Rectangle::new(rect.loc, Size::from((rect.size.w, first_h)));
            let second = Rectangle::new(
                Point::from((rect.loc.x, rect.loc.y + first_h + gaps)),
                Size::from((rect.size.w, second_h)),
            );
            (first, second)
        }
        SplitAxis::Vertical => {
            let usable = (rect.size.w - gaps).max(0.);
            let first_w = (usable * ratio).clamp(0., usable);
            let second_w = usable - first_w;
            let first = Rectangle::new(rect.loc, Size::from((first_w, rect.size.h)));
            let second = Rectangle::new(
                Point::from((rect.loc.x + first_w + gaps, rect.loc.y)),
                Size::from((second_w, rect.size.h)),
            );
            (first, second)
        }
    }
}

/// Picks the default slice side for a region of the given size: wide regions split side-by-side
/// (new window on the right), while tall or square regions stack (new window at the bottom).
///
/// The focused window always keeps the `First` (top-left) half and shrinks into its corner, while
/// the newly opened window takes the freed-up half, mirroring Hyprland's dwindle (force_split 2).
fn default_side_for_aspect(size: Size<f64, Logical>) -> SplitSide {
    if size.w > size.h {
        SplitSide::Right
    } else {
        SplitSide::Bottom
    }
}

/// Recursively opens a new leaf next to the leaf at `path`, rebuilding only the nodes along the
/// path.
///
/// Returns the new subtree and the path of the newly inserted leaf.
fn open_leaf_at<T>(
    node: Node<T>,
    path: &[Child],
    side: SplitSide,
    value: T,
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
                    ratio: DEFAULT_RATIO,
                    first,
                    second,
                },
                LeafPath(vec![new_child]),
            )
        }
        (Node::Leaf(old), Some(_)) => {
            // Path points deeper than the tree goes; clamp to the leaf itself.
            open_leaf_at(Node::Leaf(old), &[], side, value)
        }
        (Node::Split { axis, ratio, first, second }, None) => {
            // Path ended at a split; insert into the first child instead.
            let (new_first, tail) = open_leaf_at(*first, &[], side, value);
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
            let (new_first, tail) = open_leaf_at(*first, &path[1..], side, value);
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
            let (new_second, tail) = open_leaf_at(*second, &path[1..], side, value);
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
        (Node::Split { first, second: _, .. }, Some(Child::First)) => {
            leaf_value_mut_of(first, &path[1..])
        }
        (Node::Split { first: _, second, .. }, Some(Child::Second)) => {
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

fn clamp_ratio(ratio: f64) -> f64 {
    f64::max(MIN_RATIO, f64::min(MAX_RATIO, ratio))
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

fn center(rect: Rectangle<f64, Logical>) -> Point<f64, Logical> {
    Point::from((
        rect.loc.x + rect.size.w / 2.,
        rect.loc.y + rect.size.h / 2.,
    ))
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/// Computes the distance and overlap score of `candidate` relative to `from` in direction `dir`.
///
/// Returns `None` when the candidate is not strictly in that direction.
fn directional_score(
    from: Point<f64, Logical>,
    candidate: Point<f64, Logical>,
    from_rect: Rectangle<f64, Logical>,
    candidate_rect: Rectangle<f64, Logical>,
    dir: SpatialDir,
) -> Option<(f64, f64)> {
    let dx = candidate.x - from.x;
    let dy = candidate.y - from.y;
    let horizontal_overlap = f64::min(
        from_rect.loc.x + from_rect.size.w,
        candidate_rect.loc.x + candidate_rect.size.w,
    ) - f64::max(from_rect.loc.x, candidate_rect.loc.x);
    let vertical_overlap = f64::min(
        from_rect.loc.y + from_rect.size.h,
        candidate_rect.loc.y + candidate_rect.size.h,
    ) - f64::max(from_rect.loc.y, candidate_rect.loc.y);

    let (dist, overlap) = match dir {
        SpatialDir::Up if dy < 0. => (from.y - candidate.y, horizontal_overlap),
        SpatialDir::Down if dy > 0. => (candidate.y - from.y, horizontal_overlap),
        SpatialDir::Left if dx < 0. => (from.x - candidate.x, vertical_overlap),
        SpatialDir::Right if dx > 0. => (candidate.x - from.x, vertical_overlap),
        _ => return None,
    };

    Some((dist, f64::max(0., overlap)))
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
        // Using aspect-based default sides, the region of the active leaf grows wide after the
        // first split, so the second window splits side-by-side instead of stacking; the new
        // window always takes the right/bottom half, so DFS order is insertion order:
        // open_new on square regions produces H{0, V{1, H{2, 3}}}.
        let mut tree = DwindleTree::new();
        tree.open_new(0, square());
        tree.open_new(1, square());
        tree.open_new(2, square());
        tree.open_new(3, square());
        assert_eq!(tree.leaves().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        assert_axis(&tree, &[Child::First], SplitAxis::Horizontal);
        assert_axis(&tree, &[Child::Second, Child::First], SplitAxis::Vertical);
        assert_axis(&tree, &[Child::Second, Child::Second, Child::First], SplitAxis::Horizontal);
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
        assert_eq!(*ratio, DEFAULT_RATIO + 0.25);

        // A horizontal drag on B must NOT touch the vertical split (no matching divider to move
        // there), so the vertical ratio is unchanged and the outer horizontal split is adjusted.
        assert!(tree.adjust_ancestor_ratio(&b, SplitAxis::Horizontal, 20., 200.));
        let Node::Split { axis: SplitAxis::Horizontal, ratio, .. } = walk(&tree, &[]) else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO + 0.1);
        let Node::Split { axis: SplitAxis::Vertical, ratio, .. } = walk(
            &tree,
            &[Child::Second],
        ) else {
            unreachable!();
        };
        assert_eq!(*ratio, DEFAULT_RATIO + 0.25);

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
        assert_eq!(*ratio, DEFAULT_RATIO + 0.25 + 0.1);
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
        // Tree: H{0, V{2, 1}} over (0,0,1000x1000):
        //   leaf0 = (0,0,1000x500), leaf2 = (0,500,500x500), leaf1 = (500,500,500x500)
        let mut tree = DwindleTree::single(0);
        tree.open_new_on(1, SplitSide::Bottom, square());
        tree.open_new_on(2, SplitSide::Right, wide());
        let content = Rectangle::new(Point::from((0., 0.)), Size::from((1000., 1000.)));
        let gaps = 0.;

        let leaf0 = LeafPath(vec![Child::First]);
        let leaf2 = LeafPath(vec![Child::Second, Child::First]);
        let leaf1 = LeafPath(vec![Child::Second, Child::Second]);

        assert_eq!(
            tree.spatial_neighbor(&leaf1, SpatialDir::Left, content, gaps),
            Some(leaf0.clone())
        );
        assert_eq!(
            tree.spatial_neighbor(&leaf2, SpatialDir::Up, content, gaps),
            Some(leaf0.clone())
        );
        assert_eq!(
            tree.spatial_neighbor(&leaf0, SpatialDir::Down, content, gaps),
            Some(leaf2.clone())
        );
        assert_eq!(
            tree.spatial_neighbor(&leaf0, SpatialDir::Right, content, gaps),
            Some(leaf1.clone())
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
}