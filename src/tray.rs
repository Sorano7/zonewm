use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const ICON_SIZE: u32 = 16;

pub struct TrayEvents {
    pub quit: bool,
    pub refresh: bool,
    pub pause_toggled: bool,
}

pub struct SystemTray {
    _icon: TrayIcon,
    quit_id: MenuId,
    refresh_id: MenuId,
    pause_item: CheckMenuItem,
}

impl SystemTray {
    pub fn new() -> Self {
        let refresh_item = MenuItem::new("Refresh", true, None);
        let refresh_id = refresh_item.id().clone();
        let pause_item = CheckMenuItem::new("Pause", true, false, None);
        let quit_item = MenuItem::new("Quit", true, None);
        let quit_id = quit_item.id().clone();

        let menu = Menu::new();
        menu.append(&refresh_item).expect("zonewm: failed to build tray menu");
        menu.append(&pause_item).expect("zonewm: failed to build tray menu");
        menu.append(&quit_item).expect("zonewm: failed to build tray menu");

        let icon = Icon::from_rgba(icon_rgba(), ICON_SIZE, ICON_SIZE)
            .expect("zonewm: failed to build tray icon");

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ZoneWM")
            .with_icon(icon)
            .build()
            .expect("zonewm: failed to create tray icon");

        Self { _icon: icon, quit_id, refresh_id, pause_item }
    }

    /// Drains pending tray menu events.
    pub fn poll_events(&self) -> TrayEvents {
        let mut events = TrayEvents { quit: false, refresh: false, pause_toggled: false };
        let pause_id = self.pause_item.id().clone();
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit_id {
                events.quit = true;
            } else if event.id == self.refresh_id {
                events.refresh = true;
            } else if event.id == pause_id {
                events.pause_toggled = true;
            }
        }
        events
    }

    pub fn set_pause_checked(&self, paused: bool) {
        self.pause_item.set_checked(paused);
    }
}

fn icon_rgba() -> Vec<u8> {
    [0x20u8, 0x80, 0xd0, 0xff]
        .repeat((ICON_SIZE * ICON_SIZE) as usize)
}
