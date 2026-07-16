//! Shared UI components and styling used across the launcher.

use makepad_widgets::ScriptVm;

pub mod styles;

pub fn script_mod(vm: &mut ScriptVm) {
    styles::script_mod(vm);
}
