//! The launcher's home screen: paged app grid, page indicator, app drawer, and edit chrome.

use makepad_widgets::ScriptVm;

pub mod app_store;
pub mod create_bar;
pub mod home_pager;
pub mod home_screen;
pub mod page_indicator;
pub mod agent_console;
pub mod app_drawer;
pub mod context_menu;
pub mod dock;
pub mod notif_badge;
pub mod search_overlay;
pub mod app_info;
pub mod import_modal;
pub mod permissions_page;
pub mod providers_page;
pub mod source_modal;

pub fn script_mod(vm: &mut ScriptVm) {
    // Order matters here, as some widget definitions depend on others.
    notif_badge::script_mod(vm);
    home_pager::script_mod(vm);
    page_indicator::script_mod(vm);
    agent_console::script_mod(vm);
    app_drawer::script_mod(vm);
    context_menu::script_mod(vm);
    app_info::script_mod(vm);
    source_modal::script_mod(vm);
    app_store::script_mod(vm);
    import_modal::script_mod(vm);
    permissions_page::script_mod(vm);
    providers_page::script_mod(vm);
    dock::script_mod(vm);
    search_overlay::script_mod(vm);
    create_bar::script_mod(vm);
    home_screen::script_mod(vm);
}
