//! The launcher's home screen: paged app grid, page indicator, app drawer, and edit chrome.

use makepad_widgets::ScriptVm;

pub mod app_store;
pub mod home_pager;
pub mod home_screen;
pub mod page_indicator;
pub mod app_drawer;
pub mod context_menu;
pub mod dock;
pub mod notif_badge;
pub mod search_overlay;

pub fn script_mod(vm: &mut ScriptVm) {
    // Order matters here, as some widget definitions depend on others.
    notif_badge::script_mod(vm);
    home_pager::script_mod(vm);
    page_indicator::script_mod(vm);
    app_drawer::script_mod(vm);
    context_menu::script_mod(vm);
    app_store::script_mod(vm);
    dock::script_mod(vm);
    search_overlay::script_mod(vm);
    home_screen::script_mod(vm);
}
