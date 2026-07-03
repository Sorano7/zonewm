use crate::models::monitor::Rect;

pub const MAX_POS_DELTA: i32       = 200;
pub const MAX_SIZE_DELTA: i32      = 200;
pub const AUTO_SNAP_THRESHOLD: i32 = 200;

/// A zone in normalised coordinates; all fields are fractions of the monitor
#[derive(Debug, Clone)]
pub struct Zone {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Zone {
    pub fn to_rect(&self, work_area: Rect) -> Rect {
        let ww = work_area.width() as f32;
        let wh = work_area.height() as f32;
        Rect {
            left:   work_area.left + (self.x * ww) as i32,
            top:    work_area.top  + (self.y * wh) as i32,
            right:  work_area.left + ((self.x + self.w) * ww) as i32,
            bottom: work_area.top  + ((self.y + self.h) * wh) as i32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum ZoneNode {
    /// Index into the owning `Layout`'s flat `zones` list.
    Leaf(usize),
    Split { axis: Axis, children: Vec<ZoneNode> },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reach {
    pub neg_h: u32,
    pub pos_h: u32,
    pub neg_v: u32,
    pub pos_v: u32,
}

impl Reach {
    /// Extends the edge on the `axis`/`forward` side outward by one level.
    pub fn grow(&mut self, axis: Axis, forward: bool) {
        match (axis, forward) {
            (Axis::Horizontal, true)  => self.pos_h += 1,
            (Axis::Horizontal, false) => self.neg_h += 1,
            (Axis::Vertical,   true)  => self.pos_v += 1,
            (Axis::Vertical,   false) => self.neg_v += 1,
        }
    }

    /// Releases one level from the edge *opposite* the `axis`/`forward` side.
    pub fn shrink(&mut self, axis: Axis, forward: bool) {
        match (axis, forward) {
            (Axis::Horizontal, true)  => self.neg_h = self.neg_h.saturating_sub(1),
            (Axis::Horizontal, false) => self.pos_h = self.pos_h.saturating_sub(1),
            (Axis::Vertical,   true)  => self.neg_v = self.neg_v.saturating_sub(1),
            (Axis::Vertical,   false) => self.pos_v = self.pos_v.saturating_sub(1),
        }
    }
}

impl ZoneNode {
    fn contains_leaf(&self, leaf_idx: usize) -> bool {
        match self {
            ZoneNode::Leaf(i) => *i == leaf_idx,
            ZoneNode::Split { children, .. } => children.iter().any(|c| c.contains_leaf(leaf_idx)),
        }
    }

    fn collect_leaves(&self, out: &mut Vec<usize>) {
        match self {
            ZoneNode::Leaf(i) => out.push(*i),
            ZoneNode::Split { children, .. } => children.iter().for_each(|c| c.collect_leaves(out)),
        }
    }

    fn sibling_bbox(children: &[ZoneNode], idx: usize, zones: &[Zone], work_area: Rect) -> Rect {
        let mut leaves = Vec::new();
        children[idx].collect_leaves(&mut leaves);
        Rect::union(&leaves.iter().map(|&i| zones[i].to_rect(work_area)).collect::<Vec<_>>())
    }

    fn bounds(&self, leaf_idx: usize, budget: &mut Reach, zones: &[Zone], work_area: Rect) -> Option<Rect> {
        match self {
            ZoneNode::Leaf(i) if *i == leaf_idx => Some(zones[leaf_idx].to_rect(work_area)),
            ZoneNode::Leaf(_) => None,
            ZoneNode::Split { axis, children } => {
                let child_idx = children.iter().position(|c| c.contains_leaf(leaf_idx))?;
                let mut rect = children[child_idx].bounds(leaf_idx, budget, zones, work_area)?;

                let (neg, pos) = match axis {
                    Axis::Horizontal => (&mut budget.neg_h, &mut budget.pos_h),
                    Axis::Vertical    => (&mut budget.neg_v, &mut budget.pos_v),
                };

                let mut idx = child_idx;
                while *pos > 0 && idx + 1 < children.len() {
                    idx += 1;
                    let bbox = Self::sibling_bbox(children, idx, zones, work_area);
                    rect = match axis {
                        Axis::Horizontal => Rect { right:  bbox.right,  ..rect },
                        Axis::Vertical    => Rect { bottom: bbox.bottom, ..rect },
                    };
                    *pos -= 1;
                }

                let mut idx = child_idx;
                while *neg > 0 && idx > 0 {
                    idx -= 1;
                    let bbox = Self::sibling_bbox(children, idx, zones, work_area);
                    rect = match axis {
                        Axis::Horizontal => Rect { left: bbox.left, ..rect },
                        Axis::Vertical    => Rect { top:  bbox.top,  ..rect },
                    };
                    *neg -= 1;
                }

                Some(rect)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Layout {
    #[allow(unused)]
    pub name: String,
    pub zones: Vec<Zone>,
    pub tree: ZoneNode,
}

impl Layout {
    /// Rect `leaf_idx` covers once stretched by `reach`.
    pub fn bounds_for_reach(&self, leaf_idx: usize, reach: Reach, work_area: Rect) -> Rect {
        let mut budget = reach;
        self.tree.bounds(leaf_idx, &mut budget, &self.zones, work_area)
            .unwrap_or_else(|| self.zones[leaf_idx].to_rect(work_area))
    }
}

#[cfg(test)]
mod test {
    use crate::{models::{monitor::Rect, zone::{Axis, Layout, Reach, Zone, ZoneNode}}, test_utils::{two_by_two_layout, two_col_layout, work_area}};

    #[test]
    fn zone_to_rect_fills_work_area() {
        let zone = Zone { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };
        assert_eq!(zone.to_rect(work_area()), work_area());
    }

    #[test]
    fn zone_to_rect_left_half() {
        let zone = Zone { x: 0.0, y: 0.0, w: 0.5, h: 1.0 };
        assert_eq!(zone.to_rect(work_area()), Rect { left: 0, top: 0, right: 960, bottom: 1080 });
    }

    #[test]
    fn zone_to_rect_top_right_quarter() {
        let zone = Zone { x: 0.5, y: 0.0, w: 0.5, h: 0.5 };
        assert_eq!(zone.to_rect(work_area()), Rect { left: 960, top: 0, right: 1920, bottom: 540 });
    }

    #[test]
    fn unstretched_reach_is_the_leaf_own_rect() {
        let layout = two_col_layout();
        assert_eq!(
            layout.bounds_for_reach(0, Reach::default(), work_area()),
            Rect { left: 0, top: 0, right: 960, bottom: 1080 },
        );
    }

    #[test]
    fn growing_right_from_left_column_covers_whole_layout() {
        let layout = two_col_layout();
        let mut reach = Reach::default();
        reach.grow(Axis::Horizontal, true);
        assert_eq!(layout.bounds_for_reach(0, reach, work_area()), work_area());
    }

    #[test]
    fn growing_left_from_leftmost_leaf_is_a_no_op() {
        let layout = two_col_layout();
        let mut reach = Reach::default();
        reach.grow(Axis::Horizontal, false);
        assert_eq!(
            layout.bounds_for_reach(0, reach, work_area()),
            layout.bounds_for_reach(0, Reach::default(), work_area()),
        );
    }

    #[test]
    fn growing_along_unrelated_axis_is_a_no_op() {
        // two_col_layout only splits horizontally; Up/Down has no ancestor to climb to.
        let layout = two_col_layout();
        let mut reach = Reach::default();
        reach.grow(Axis::Vertical, true);
        assert_eq!(
            layout.bounds_for_reach(0, reach, work_area()),
            layout.bounds_for_reach(0, Reach::default(), work_area()),
        );
    }

    #[test]
    fn growing_in_2x2_grid_consumes_only_the_matching_axis_sibling() {
        let layout = two_by_two_layout();
        let mut right = Reach::default();
        right.grow(Axis::Horizontal, true);
        assert_eq!(layout.bounds_for_reach(0, right, work_area()), Rect { left: 0, top: 0, right: 1920, bottom: 540 });

        let mut down = Reach::default();
        down.grow(Axis::Vertical, true);
        assert_eq!(layout.bounds_for_reach(0, down, work_area()), Rect { left: 0, top: 0, right: 960, bottom: 1080 });
    }

    #[test]
    fn growing_consumes_entire_subdivided_sibling_subtree() {
        let layout = Layout {
            name: "left-1-right-2".into(),
            zones: vec![
                Zone { x: 0.0, y: 0.0, w: 0.5, h: 1.0 },
                Zone { x: 0.5, y: 0.0, w: 0.5, h: 0.5 },
                Zone { x: 0.5, y: 0.5, w: 0.5, h: 0.5 },
            ],
            tree: ZoneNode::Split {
                axis: Axis::Horizontal,
                children: vec![
                    ZoneNode::Leaf(0),
                    ZoneNode::Split { axis: Axis::Vertical, children: vec![ZoneNode::Leaf(1), ZoneNode::Leaf(2)] },
                ],
            },
        };

        let mut reach = Reach::default();
        reach.grow(Axis::Horizontal, true);
        assert_eq!(layout.bounds_for_reach(0, reach, work_area()), work_area());
    }

    #[test]
    fn growing_from_partial_leaf_keeps_its_own_cross_axis_bounds() {
        let layout = Layout {
            name: "left-2-right-1".into(),
            zones: vec![
                Zone { x: 0.0, y: 0.0, w: 0.5, h: 0.5 },
                Zone { x: 0.0, y: 0.5, w: 0.5, h: 0.5 },
                Zone { x: 0.5, y: 0.0, w: 0.5, h: 1.0 },
            ],
            tree: ZoneNode::Split {
                axis: Axis::Horizontal,
                children: vec![
                    ZoneNode::Split { axis: Axis::Vertical, children: vec![ZoneNode::Leaf(0), ZoneNode::Leaf(1)] },
                    ZoneNode::Leaf(2),
                ],
            },
        };

        let mut reach = Reach::default();
        reach.grow(Axis::Horizontal, true);
        assert_eq!(layout.bounds_for_reach(0, reach, work_area()), Rect { left: 0, top: 0, right: 1920, bottom: 540 });
    }

    #[test]
    fn repeated_growth_climbs_one_more_ancestor_each_time() {
        // 3-column layout; growing the leftmost leaf right twice should
        // consume one column per step, ending by covering the whole area.
        let layout = Layout {
            name: "3-col".into(),
            zones: vec![
                Zone { x: 0.0,     y: 0.0, w: 1.0 / 3.0, h: 1.0 },
                Zone { x: 1.0/3.0, y: 0.0, w: 1.0 / 3.0, h: 1.0 },
                Zone { x: 2.0/3.0, y: 0.0, w: 1.0 / 3.0, h: 1.0 },
            ],
            tree: ZoneNode::Split {
                axis: Axis::Horizontal,
                children: vec![ZoneNode::Leaf(0), ZoneNode::Leaf(1), ZoneNode::Leaf(2)],
            },
        };

        let mut reach = Reach::default();
        reach.grow(Axis::Horizontal, true);
        let one_step = layout.bounds_for_reach(0, reach, work_area());
        assert_eq!(one_step, Rect { left: 0, top: 0, right: 1280, bottom: 1080 });

        reach.grow(Axis::Horizontal, true);
        let two_steps = layout.bounds_for_reach(0, reach, work_area());
        assert_eq!(two_steps, work_area());
    }

    #[test]
    fn shrink_releases_the_opposite_edge_toward_the_given_direction() {
        let layout = two_col_layout();
        let mut reach = Reach::default();
        reach.grow(Axis::Horizontal, true); // window 0 now covers the whole area
        assert_eq!(layout.bounds_for_reach(0, reach, work_area()), work_area());

        // Shrinking "left" releases the rightward growth, back to zone 0's own rect.
        reach.shrink(Axis::Horizontal, false);
        assert_eq!(
            layout.bounds_for_reach(0, reach, work_area()),
            Rect { left: 0, top: 0, right: 960, bottom: 1080 },
        );
    }

    #[test]
    fn shrink_below_zero_is_a_no_op() {
        let mut reach = Reach::default();
        reach.shrink(Axis::Horizontal, false);
        assert_eq!(reach, Reach::default());
    }
}
