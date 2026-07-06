use windows::Win32::Foundation::HWND;

pub const WORKSPACE_COUNT: usize = 9;

#[derive(Default)]
pub struct LastFocused {
    pub window: Option<HWND>,
    pub zone: Option<usize>,
}

pub struct Workspace {
    pub layout_idx: usize,
    /// Zoned windows. Index = zone slot; inner Vec = Z-ordered windows
    /// (index 0 = bottom, last = top).
    pub zoned: Vec<Vec<HWND>>,
    /// Windows that belong to this workspace but aren't snapped to any zone.
    pub floating: Vec<HWND>,
    pub last_focused: LastFocused,
    /// Slot for fullscreen display.
    pub fullscreen: Option<HWND>,
}

impl Workspace {
    pub fn new(zone_count: usize) -> Self {
        Self { 
            layout_idx: 0, 
            zoned: vec![vec![]; zone_count], 
            floating: Vec::new(), 
            last_focused: LastFocused::default(),
            fullscreen: None,
        }
    }

    pub fn all_windows(&self) -> Vec<HWND> {
        self.zoned.iter().flatten().copied()
            .chain(self.floating.iter().copied())
            .collect()
    }

    pub fn remove(&mut self, hwnd: HWND) {
        for zone in &mut self.zoned {
            zone.retain(|&h| h != hwnd);
        }
        self.floating.retain(|&h| h != hwnd);
    }

    pub fn get_zone_index(&self, hwnd: HWND) -> Option<usize> {
        self.zoned.iter().position(|z| z.contains(&hwnd))
    }
}
