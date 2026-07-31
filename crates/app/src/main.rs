// Everything lives in the library; this only starts it. Tauri's mobile entry
// point needs a library target, and a binary-only crate cannot be tested from
// the outside.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    nirmoka_app::run()
}
