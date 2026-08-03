use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const ICON_SIZE: u32 = 16;

pub struct SystemTray {
    _icon: TrayIcon,
    quit_id: MenuId,
    refresh_id: MenuId,
}

impl SystemTray {
    pub fn new() -> Self {
        let refresh_item = MenuItem::new("Refresh", true, None);
        let refresh_id = refresh_item.id().clone();
        let quit_item = MenuItem::new("Quit", true, None);
        let quit_id = quit_item.id().clone();

        let menu = Menu::new();
        menu.append(&refresh_item).expect("zonewm: failed to build tray menu");
        menu.append(&quit_item).expect("zonewm: failed to build tray menu");

        let icon = Icon::from_rgba(icon_rgba(), ICON_SIZE, ICON_SIZE)
            .expect("zonewm: failed to build tray icon");

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ZoneWM")
            .with_icon(icon)
            .build()
            .expect("zonewm: failed to create tray icon");

        Self { _icon: icon, quit_id, refresh_id }
    }

    /// Drains pending tray menu events, returning `(quit_requested, refresh_requested)`.
    pub fn poll_events(&self) -> (bool, bool) {
        let mut quit = false;
        let mut refresh = false;
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit_id {
                quit = true;
            } else if event.id == self.refresh_id {
                refresh = true;
            }
        }
        (quit, refresh)
    }
}

fn icon_rgba() -> Vec<u8> {
    [0x20u8, 0x80, 0xd0, 0xff]
        .repeat((ICON_SIZE * ICON_SIZE) as usize)
}
