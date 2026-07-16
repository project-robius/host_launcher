//! Mini-apps: registry of installed apps, and the screens/tiles that host running instances.

pub mod builtin;
pub mod registry;
pub mod mini_app_screen;

pub fn script_mod(vm: &mut makepad_widgets::ScriptVm) {
    mini_app_screen::script_mod(vm);
}
