use windows::Win32::Foundation::HWND;

use crate::{models::window, state::{StateMap, monitor_state::MonitorState, window_state::{WindowRecord, WindowState}}};

pub fn enable_ansi_console() {
    use windows::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode,
        CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
    };
    unsafe {
        if let Ok(handle) = GetStdHandle(STD_OUTPUT_HANDLE) {
            let mut mode = CONSOLE_MODE(0);
            if GetConsoleMode(handle, &mut mode).is_ok() {
                let _ = SetConsoleMode(handle, CONSOLE_MODE(mode.0 | ENABLE_VIRTUAL_TERMINAL_PROCESSING.0));
            }
        }
    }
}

pub fn print_status(states: &StateMap, focused: HWND) {
    print!("\x1b[2J\x1b[H");

    let mut monitors: Vec<&MonitorState> = states.values().collect();
    monitors.sort_by_key(|ms| (ms.monitor.work_area.left, ms.monitor.work_area.top));

    println!("  {:<4} {:<4} {:<6} {:<3} {:<8} {}", "Mon", "WS", "Zone", "Z", "State", "Title");
    println!("  {}", "-".repeat(72));

    for (mon_idx, ms) in monitors.iter().enumerate() {
        let mut records: Vec<WindowRecord> = ms.all_window_records();
        records.sort_by_key(|r| {
            let zone_key = match r.state { WindowState::Zoned(z) => z, _ => usize::MAX };
            (r.ws_idx, zone_key, r.z_order)
        });

        for r in &records {
            let focus = if r.hwnd == focused { "*" } else { " " };
            let zone_str = match r.state { WindowState::Zoned(z) => z.to_string(), _ => "-".to_string() };
            let state_str = match r.state { WindowState::Zoned(_) => "zoned", _ => "float" };
            let title: String = window::title(r.hwnd).unwrap_or_default().chars().take(50).collect();
            println!("{} {:<4} {:<4} {:<6} {:<3} {:<8} {}",
                focus, mon_idx + 1, r.ws_idx + 1, zone_str, r.z_order, state_str, title);
        }
    }
}
