// در نسخهٔ release ویندوز، پنجرهٔ کنسول اضافی باز نشود
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dehkhoda_app_lib::run()
}
