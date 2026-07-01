use windows::Win32::Foundation::HWND;

use crate::models::monitor::Rect;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindowState {
    /// Managed but not belongs to any zone
    Floating,
    /// Managed and belongs to a zone
    Zoned(usize),
    /// Not managed
    Ignored,
}

pub struct WindowRecord {
    pub hwnd: HWND,
    pub ws_idx: usize,
    pub state: WindowState,
    pub z_order: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

impl Direction {
    pub fn from_idx(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Direction::Left),
            1 => Some(Direction::Down),
            2 => Some(Direction::Up),
            3 => Some(Direction::Right),
            _ => None,
        }
    }
}

/// Percentage of overlap at which a rect is excluded from directional movement.
const OVERLAP_EXCLUDE_PCT: f64 = 90.0;

/// Percentage of the smaller of `a` and `b`'s area that the two rects overlap.
fn overlap_pct(a: Rect, b: Rect) -> f64 {
    let ix = (a.right.min(b.right) - a.left.max(b.left)).max(0) as i64;
    let iy = (a.bottom.min(b.bottom) - a.top.max(b.top)).max(0) as i64;
    if ix == 0 || iy == 0 { return 0.0; }
    let area_a = a.width() as i64 * a.height() as i64;
    let area_b = b.width() as i64 * b.height() as i64;
    (ix * iy) as f64 / area_a.min(area_b).max(1) as f64 * 100.0
}

/// Returns the score as (travel_gap, -perp_overlap).
fn edge_score(from: Rect, r: Rect, dir: Direction) -> Option<(i32, i32)> {
    let (perp_overlap, reaches, travel_gap) = match dir {
        Direction::Left => (
            r.bottom.min(from.bottom) - r.top.max(from.top),
            r.left < from.left,
            from.left - r.right,
        ),
        Direction::Right => (
            r.bottom.min(from.bottom) - r.top.max(from.top),
            r.right > from.right,
            r.left - from.right,
        ),
        Direction::Up => (
            r.right.min(from.right) - r.left.max(from.left),
            r.top < from.top,
            from.top - r.bottom,
        ),
        Direction::Down => (
            r.right.min(from.right) - r.left.max(from.left),
            r.bottom > from.bottom,
            r.top - from.bottom,
        ),
    };
    if !reaches || perp_overlap <= 0 { return None; }
    Some((travel_gap, -perp_overlap))
}

/// Returns the nearest candidate from `from` in direction `dir`.
pub fn nearest_in_dir<T: Copy>(candidates: &[(T, Rect)], from: Rect, dir: Direction) -> Option<T> {
    candidates.iter()
        .filter(|&&(_, r)| overlap_pct(from, r) <= OVERLAP_EXCLUDE_PCT)
        .filter_map(|&(t, r)| edge_score(from, r, dir).map(|s| (t, s)))
        .min_by_key(|&(_, s)| s)
        .map(|(t, _)| t)
}

#[cfg(test)]
mod test {
    use crate::{config::to_layouts, models::{monitor::Rect, zone::Zone}, state::window_state::{Direction, nearest_in_dir}, test_utils::{h, work_area}};

    #[test]
    fn nearest_in_dir_works_with_zone_slot_candidates() {
        let candidates = [
            ((1isize, 0usize), Rect { left: 1000, top: 0, right: 1920, bottom: 1080 }),
            ((1isize, 1usize), Rect { left: 500,  top: 0, right: 600,  bottom: 1080 }),
        ];
        let from = Rect { left: 0, top: 0, right: 500, bottom: 1080 };
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Right), Some((1, 1)));
    }

    #[test]
    fn nearest_window_in_dir_maps_to_hwnd() {
        let candidates = [
            (h(10), Rect { left: 1000, top: 0, right: 1920, bottom: 1080 }),
            (h(20), Rect { left: 500,  top: 0, right: 600,  bottom: 1080 }),
        ];
        let from = Rect { left: 0, top: 0, right: 500, bottom: 1080 };
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Right), Some(h(20)));
    }

    #[test]
    fn nearest_window_in_dir_prefers_smaller_gap_over_larger_shared_edge() {
        let candidates = [
            // Shares the full height (edge share 1080) but sits farther away.
            (h(10), Rect { left: 600, top: 0,   right: 1100, bottom: 1080 }),
            // Closer, but shares only a small band (edge share 300).
            (h(20), Rect { left: 510, top: 0,   right: 700,  bottom: 300 }),
        ];
        let from = Rect { left: 0, top: 0, right: 500, bottom: 1080 };
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Right), Some(h(20)));
    }

    #[test]
    fn nearest_window_in_dir_prefers_negative_gap_over_flush_zone_with_full_edge_share() {
        let candidates = [
            // A neighbouring zone: flush (gap 0) and shares the full edge.
            (h(10), Rect { left: 500, top: 0, right: 1000, bottom: 1080 }),
            // A floating window grazing back across the boundary (gap negative)
            // but sharing only a small band of the edge.
            (h(20), Rect { left: 480, top: 0, right: 700, bottom: 200 }),
        ];
        let from = Rect { left: 0, top: 0, right: 500, bottom: 1080 };
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Right), Some(h(20)));
    }

    #[test]
    fn nearest_window_in_dir_prefers_smaller_gap_when_shared_edge_ties() {
        let candidates = [
            // Flush against the focused rect (gap 0).
            (h(10), Rect { left: 500, top: 0, right: 700, bottom: 1080 }),
            // Grazes back across the boundary without overlapping enough to be
            // excluded (gap negative, i.e. "closer" than flush).
            (h(20), Rect { left: 480, top: 0, right: 700, bottom: 1080 }),
        ];
        let from = Rect { left: 0, top: 0, right: 500, bottom: 1080 };
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Right), Some(h(20)));
    }

    #[test]
    fn nearest_window_in_dir_excludes_physically_overlapping_candidate() {
        let candidates = [
            // Mostly overlaps `from` (e.g. a floating window sitting on top of it).
            (h(10), Rect { left: 100, top: 100, right: 400, bottom: 900 }),
            // Genuinely adjacent, further away.
            (h(20), Rect { left: 500, top: 0,   right: 700, bottom: 1080 }),
        ];
        let from = Rect { left: 0, top: 0, right: 500, bottom: 1080 };
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Right), Some(h(20)));
    }

    #[test]
    fn nearest_window_in_dir_none_when_only_candidate_overlaps() {
        let candidates = [
            (h(10), Rect { left: 100, top: 100, right: 400, bottom: 900 }),
        ];
        let from = Rect { left: 0, top: 0, right: 500, bottom: 1080 };
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Right), None);
    }

    fn visible_rect_for_zone(zone: Rect) -> Rect {
        Rect {
            left:   zone.left   + crate::window::SNAP_GAP,
            top:    zone.top    + crate::window::SNAP_GAP,
            right:  zone.right  - crate::window::SNAP_GAP,
            bottom: zone.bottom - crate::window::SNAP_GAP,
        }
    }

    fn two_by_two_zones() -> Vec<Zone> {
        let cfg: crate::config::Config = toml::from_str(r#"
            [[layout]]
            name = "2x2"
            zones = { columns = [0.5, 0.5], children = [
                { rows = [0.5, 0.5] },
                { rows = [0.5, 0.5] },
            ]}
        "#).unwrap();
        to_layouts(&cfg)[0].as_ref().unwrap().zones.clone()
    }

    #[test]
    fn focus_up_from_bottom_right_reaches_top_right() {
        let zones = two_by_two_zones();
        let work = work_area();
        let (tl, bl, tr, br) = (
            zones[0].to_rect(work), zones[1].to_rect(work),
            zones[2].to_rect(work), zones[3].to_rect(work),
        );
        let candidates = [(h(1), tl), (h(2), bl), (h(3), tr)];
        let from = visible_rect_for_zone(br);
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Up), Some(h(3)));
    }

    #[test]
    fn focus_down_from_top_left_is_noop_when_bottom_left_empty() {
        let zones = two_by_two_zones();
        let work = work_area();
        let (tl, tr, br) = (zones[0].to_rect(work), zones[2].to_rect(work), zones[3].to_rect(work));
        let candidates = [(h(3), tr), (h(4), br)];
        let from = visible_rect_for_zone(tl);
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Down), None);
    }

    #[test]
    fn focus_up_from_bottom_right_is_noop_when_top_right_empty() {
        let zones = two_by_two_zones();
        let work = work_area();
        let (tl, br) = (zones[0].to_rect(work), zones[3].to_rect(work));
        let candidates = [(h(1), tl)];
        let from = visible_rect_for_zone(br);
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Up), None);
    }

    #[test]
    fn focus_down_from_top_left_is_noop_when_only_diagonal_occupied() {
        let zones = two_by_two_zones();
        let work = work_area();
        let (tl, br) = (zones[0].to_rect(work), zones[3].to_rect(work));
        let candidates = [(h(4), br)];
        let from = visible_rect_for_zone(tl);
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Down), None);
    }

    #[test]
    fn focus_right_from_top_left_is_noop_when_only_diagonal_occupied() {
        let zones = two_by_two_zones();
        let work = work_area();
        let (tl, br) = (zones[0].to_rect(work), zones[3].to_rect(work));
        let candidates = [(h(4), br)];
        let from = visible_rect_for_zone(tl);
        assert_eq!(nearest_in_dir(&candidates, from, Direction::Right), None);
    }
}
