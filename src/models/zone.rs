use crate::models::monitor::Rect;

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

#[derive(Debug, Clone)]
pub struct Layout {
    pub name: String,
    pub zones: Vec<Zone>,
}

#[cfg(test)]
mod test {
    use crate::{models::{monitor::Rect, zone::Zone}, test_utils::work_area};
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
}
