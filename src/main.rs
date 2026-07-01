#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use zonewm::run_wm;

fn main() {
    run_wm();
}
