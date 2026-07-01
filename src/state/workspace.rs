use windows::Win32::Foundation::HWND;

pub const WORKSPACE_COUNT: usize = 9;


pub struct Workspace {
    pub layout_idx: usize,
    /// Zoned windows. Index = zone slot; inner Vec = Z-ordered windows
    /// (index 0 = bottom, last = top).
    pub zoned: Vec<Vec<HWND>>,
    /// Windows that belong to this workspace but aren't snapped to any zone.
    pub floating: Vec<HWND>,
}

impl Workspace {
    pub fn new(zone_count: usize) -> Self {
        Self { layout_idx: 0, zoned: vec![vec![]; zone_count], floating: Vec::new() }
    }

    pub fn all_windows(&self) -> Vec<HWND> {
        self.zoned.iter().flatten().copied()
            .chain(self.floating.iter().copied())
            .collect()
    }

    pub fn contains(&self, hwnd: HWND) -> bool {
        self.zoned.iter().any(|z| z.contains(&hwnd)) || self.floating.contains(&hwnd)
    }

    pub fn remove(&mut self, hwnd: HWND) {
        for zone in &mut self.zoned {
            zone.retain(|&h| h != hwnd);
        }
        self.floating.retain(|&h| h != hwnd);
    }
}
