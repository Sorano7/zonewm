use std::cell::{Cell, RefCell};

use windows_core::{interface, Interface, GUID, HRESULT, IUnknown, IUnknown_Vtbl};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IServiceProvider,
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
};

// {C2F03A33-21F5-47FA-B4BB-156362A2F239}
const CLSID_IMMERSIVE_SHELL: GUID = GUID {
    data1: 0xC2F03A33,
    data2: 0x21F5,
    data3: 0x47FA,
    data4: [0xB4, 0xBB, 0x15, 0x63, 0x62, 0xA2, 0xF2, 0x39],
};

// 3 filler slots before get_view_for_hwnd (after the 3 IUnknown slots)
#[interface("1841c6d7-4f9d-42c0-af41-8747538f10e5")]
unsafe trait IApplicationViewCollection: IUnknown {
    unsafe fn m1(&self);
    unsafe fn m2(&self);
    unsafe fn m3(&self);
    unsafe fn get_view_for_hwnd(
        &self,
        window: isize,
        view: *mut Option<IApplicationView>,
    ) -> HRESULT;
}

// 9 filler slots before set_cloak (after the 3 IUnknown slots)
#[interface("372E1D3B-38D3-42E4-A15B-8AB2B178F513")]
unsafe trait IApplicationView: IUnknown {
    unsafe fn m1(&self);
    unsafe fn m2(&self);
    unsafe fn m3(&self);
    unsafe fn m4(&self);
    unsafe fn m5(&self);
    unsafe fn m6(&self);
    unsafe fn m7(&self);
    unsafe fn m8(&self);
    unsafe fn m9(&self);
    unsafe fn set_cloak(&self, cloak_type: u32, cloak_flag: i32) -> HRESULT;
}

thread_local! {
    static COM_INIT: Cell<bool> = Cell::new(false);
    static COLLECTION: RefCell<Option<IApplicationViewCollection>> = RefCell::new(None);
}

fn ensure_com() {
    COM_INIT.with(|c| {
        if !c.get() {
            unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED); }
            c.set(true);
        }
    });
}

fn get_collection() -> windows::core::Result<IApplicationViewCollection> {
    unsafe {
        let shell: IServiceProvider =
            CoCreateInstance(&CLSID_IMMERSIVE_SHELL, None, CLSCTX_LOCAL_SERVER)?;
        shell.QueryService(&IApplicationViewCollection::IID)
    }
}

/// Cloak or uncloak a window using the undocumented IApplicationView COM chain.
/// Prefer this over ShowWindow: no taskbar flicker, no WM_ACTIVATE cascade.
pub fn set_cloak(hwnd: HWND, cloaked: bool) {
    ensure_com();
    let flag: i32 = if cloaked { 2 } else { 0 };
    unsafe {
        // Use the cached collection; on any failure, invalidate and retry once.
        // The ImmersiveShell pointer goes stale when Explorer restarts.
        let mut refreshed = false;
        loop {
            let collection = COLLECTION.with(|c| {
                let mut slot = c.borrow_mut();
                if slot.is_none() {
                    *slot = get_collection().ok();
                }
                slot.clone()
            });
            if let Some(collection) = collection {
                let mut view: Option<IApplicationView> = None;
                if collection.get_view_for_hwnd(hwnd.0 as isize, &mut view).is_ok() {
                    if let Some(v) = view {
                        let _ = v.set_cloak(1, flag);
                        return;
                    }
                }
            }
            if refreshed { break; }
            COLLECTION.with(|c| c.borrow_mut().take());
            refreshed = true;
        }
    }
}

pub fn is_cloaked(hwnd: HWND) -> bool {
    let mut val: u32 = 0;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut val as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
        .is_ok()
            && val != 0
    }
}
