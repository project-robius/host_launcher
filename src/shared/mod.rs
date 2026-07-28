//! Shared UI components and styling used across the launcher.

use makepad_widgets::ScriptVm;

pub mod expand_arrow;
pub mod styles;

pub fn script_mod(vm: &mut ScriptVm) {
    expand_arrow::script_mod(vm);
    styles::script_mod(vm);
}
