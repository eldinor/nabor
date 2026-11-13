#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if nabor_lib::document_worker_main() {
        return;
    }
    nabor_lib::run()
}
