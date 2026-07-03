pub const DEFAULT_CONFIG: &str = r#"
# ZoneWM Configuration

# ---Layouts---

[[layout]]
name = "2-Column"
zones = { columns = [0.5, 0.5] }

[[layout]]
name = "3-Column"
zones = { columns = [0.333, 0.334, 0.333] }

[[layout]]
name = "2x2 Grid"
zones = { rows = [0.5, 0.5], children = [
    { columns = [0.5, 0.5] },
    { columns = [0.5, 0.5] },
]}


# ---Keymaps: Layout---
[[keymap]]
combo = "ctrl+alt+1"
action = "set_layout_1"
[[keymap]]
combo = "ctrl+alt+2"
action = "set_layout_2"
[[keymap]]
combo = "ctrl+alt+3"
action = "set_layout_3"
[[keymap]]
combo = "ctrl+alt+4"
action = "set_layout_4"
[[keymap]]
combo = "ctrl+alt+5"
action = "set_layout_5"
[[keymap]]
combo = "ctrl+alt+6"
action = "set_layout_6"
[[keymap]]
combo = "ctrl+alt+7"
action = "set_layout_7"
[[keymap]]
combo = "ctrl+alt+8"
action = "set_layout_8"
[[keymap]]
combo = "ctrl+alt+9"
action = "set_layout_9"

# ---Keymaps: Workspace---
[[keymap]]
combo = "alt+1"
action = "set_workspace_1"
[[keymap]]
combo = "alt+2"
action = "set_workspace_2"
[[keymap]]
combo = "alt+3"
action = "set_workspace_3"
[[keymap]]
combo = "alt+4"
action = "set_workspace_4"
[[keymap]]
combo = "alt+5"
action = "set_workspace_5"
[[keymap]]
combo = "alt+6"
action = "set_workspace_6"
[[keymap]]
combo = "alt+7"
action = "set_workspace_7"
[[keymap]]
combo = "alt+8"
action = "set_workspace_8"
[[keymap]]
combo = "alt+9"
action = "set_workspace_9"

# ---Keymaps: Move To Workspace---
[[keymap]]
combo = "alt+shift+1"
action = "move_to_workspace_1"
[[keymap]]
combo = "alt+shift+2"
action = "move_to_workspace_2"
[[keymap]]
combo = "alt+shift+3"
action = "move_to_workspace_3"
[[keymap]]
combo = "alt+shift+4"
action = "move_to_workspace_4"
[[keymap]]
combo = "alt+shift+5"
action = "move_to_workspace_5"
[[keymap]]
combo = "alt+shift+6"
action = "move_to_workspace_6"
[[keymap]]
combo = "alt+shift+7"
action = "move_to_workspace_7"
[[keymap]]
combo = "alt+shift+8"
action = "move_to_workspace_8"
[[keymap]]
combo = "alt+shift+9"
action = "move_to_workspace_9"

# ---Keymaps: Focus---
[[keymap]]
combo = "alt+h"
action = "move_focus_left"
[[keymap]]
combo = "alt+j"
action = "move_focus_down"
[[keymap]]
combo = "alt+k"
action = "move_focus_up"
[[keymap]]
combo = "alt+l"
action = "move_focus_right"

# ---Keymaps: Move/Swap---
[[keymap]]
combo = "alt+shift+h"
action = "move_window_left"
[[keymap]]
combo = "alt+shift+j"
action = "move_window_down"
[[keymap]]
combo = "alt+shift+k"
action = "move_window_up"
[[keymap]]
combo = "alt+shift+l"
action = "move_window_right"

[[keymap]]
combo = "win+left"
action = "move_window_left"
[[keymap]]
combo = "win+down"
action = "move_window_down"
[[keymap]]
combo = "win+up"
action = "move_window_up"
[[keymap]]
combo = "win+right"
action = "move_window_right"

[[keymap]]
combo = "ctrl+alt+h"
action = "swap_window_left"
[[keymap]]
combo = "ctrl+alt+j"
action = "swap_window_down"
[[keymap]]
combo = "ctrl+alt+k"
action = "swap_window_up"
[[keymap]]
combo = "ctrl+alt+l"
action = "swap_window_right"

# ---Keymaps: Cycle---
[[keymap]]
combo = "alt+n"
action = "cycle_window_next"
[[keymap]]
combo = "alt+p"
action = "cycle_window_prev"

# ---Keymaps: Stretch/Shrink---
[[keymap]]
combo = "alt+shift+u"
action = "stretch_window_left"
[[keymap]]
combo = "alt+shift+i"
action = "stretch_window_down"
[[keymap]]
combo = "alt+shift+o"
action = "stretch_window_up"
[[keymap]]
combo = "alt+shift+p"
action = "stretch_window_right"

[[keymap]]
combo = "ctrl+alt+u"
action = "shrink_window_left"
[[keymap]]
combo = "ctrl+alt+i"
action = "shrink_window_down"
[[keymap]]
combo = "ctrl+alt+o"
action = "shrink_window_up"
[[keymap]]
combo = "ctrl+alt+p"
action = "shrink_window_right"

# ---Keymaps: Extras---
[[keymap]]
combo = "alt+shift+f"
action = "set_float"

[[keymap]]
combo = "ctrl+alt+shift+g"
action = "toggle_monitor_lock"
"#;
