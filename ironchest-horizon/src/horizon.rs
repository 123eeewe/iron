//! Слой привязки к API Minecraft Horizon (ZhekMozg Mod Studio).
//!
//! Мод-рантайм Horizon компилирует скрипты мода в нативную библиотеку и
//! предоставляет ей C-экспорт `modentry`. Движок игры вызывает функции этого
//! слоя; на десктопе (для тестов ядра) вместо реальных игровых функций
//! подставляются заглушки через feature `stub`.
#![allow(non_snake_case, dead_code)]

use std::ffi::c_void;

extern "C" {
    // ---- Блоки ----
    pub fn get_block(x: i32, y: i32, z: i32) -> i32;
    pub fn set_block(x: i32, y: i32, z: i32, id: i32, data: i32);
    pub fn destroy_block(x: i32, y: i32, z: i32, drop: bool);
    pub fn get_block_name(id: i32) -> *const std::ffi::c_char;

    // ---- Предметы / сущности ----
    pub fn spawn_item(x: f32, y: f32, z: f32, name: *const std::ffi::c_char, count: i32, data: i32) -> *mut c_void;
    pub fn spawn_item_to_player(player_id: i32, name: *const std::ffi::c_char, count: i32, data: i32) -> *mut c_void;
    pub fn add_item(name: *const std::ffi::c_char, title: *const std::ffi::c_char, icon: *const std::ffi::c_char, max_stack: i32) -> i32;
    pub fn create_item_with_component(item_name: *const std::ffi::c_char, block_name: *const std::ffi::c_char, gui_texture: *const std::ffi::c_char) -> i32;

    // ---- Рецепты ----
    pub fn add_recipe(result: *const std::ffi::c_char, result_count: i32, ingredients: *const std::ffi::c_char, shapeless: bool);

    // ---- Интерфейсы ----
    pub fn open_chest_ui(title: *const std::ffi::c_char, rows: i32, callback_id: i32) -> i32;
    pub fn show_hud_message(msg: *const std::ffi::c_char, pos: i32);

    // ---- Игрок ----
    pub fn get_player_id() -> i32;
    pub fn get_targeted_block(max_dist: f32) -> i32; // id сущности блока под прицелом
}

// ================= Заглушки для десктопных тестов =================
#[cfg(feature = "stub")]
mod stubs {
    use super::*;
    use std::ffi::c_char;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::LazyLock;

    pub static LOG: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));
    pub static BLOCKS: LazyLock<Mutex<HashMap<(i32, i32, i32), (i32, i32)>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    fn log(s: String) {
        LOG.lock().unwrap().push(s);
    }

    #[no_mangle]
    pub unsafe extern "C" fn get_block(x: i32, y: i32, z: i32) -> i32 {
        BLOCKS.lock().unwrap().get(&(x, y, z)).map(|b| b.0).unwrap_or(0)
    }
    #[no_mangle]
    pub unsafe extern "C" fn set_block(x: i32, y: i32, z: i32, id: i32, data: i32) {
        log(format!("set_block({x},{y},{z},id={id},data={data})"));
        BLOCKS.lock().unwrap().insert((x, y, z), (id, data));
    }
    #[no_mangle]
    pub unsafe extern "C" fn destroy_block(x: i32, y: i32, z: i32, drop_it: bool) {
        log(format!("destroy_block({x},{y},{z},drop={drop_it})"));
        BLOCKS.lock().unwrap().remove(&(x, y, z));
    }
    #[no_mangle]
    pub unsafe extern "C" fn get_block_name(_id: i32) -> *const c_char {
        // В тестах имена блоков не используются напрямую.
        std::ptr::null()
    }
    #[no_mangle]
    pub unsafe extern "C" fn spawn_item(
        x: f32, y: f32, z: f32, name: *const c_char, count: i32, data: i32,
    ) -> *mut c_void {
        let n = if name.is_null() { String::from("?") } else { std::str::from_utf8_unchecked(std::ffi::CStr::from_ptr(name).to_bytes()).to_string() };
        log(format!("spawn_item({x},{y},{z},{n}x{count}:{data})"));
        std::ptr::null_mut()
    }
    #[no_mangle]
    pub unsafe extern "C" fn spawn_item_to_player(
        player: i32, name: *const c_char, count: i32, data: i32,
    ) -> *mut c_void {
        let n = if name.is_null() { String::from("?") } else { std::str::from_utf8_unchecked(std::ffi::CStr::from_ptr(name).to_bytes()).to_string() };
        log(format!("spawn_item_to_player(p{player},{n}x{count}:{data})"));
        std::ptr::null_mut()
    }
    #[no_mangle]
    pub unsafe extern "C" fn add_item(name: *const c_char, title: *const c_char, _icon: *const c_char, max_stack: i32) -> i32 {
        let n = std::ffi::CStr::from_ptr(name).to_string_lossy().to_string();
        let t = std::ffi::CStr::from_ptr(title).to_string_lossy().to_string();
        log(format!("add_item({n},\"{t}\",stack={max_stack})"));
        400 + n.len() as i32 % 100
    }
    #[no_mangle]
    pub unsafe extern "C" fn create_item_with_component(item: *const c_char, block: *const c_char, _tex: *const c_char) -> i32 {
        let i = std::ffi::CStr::from_ptr(item).to_string_lossy().to_string();
        let b = std::ffi::CStr::from_ptr(block).to_string_lossy().to_string();
        log(format!("create_item_with_component({i},{b})"));
        0
    }
    #[no_mangle]
    pub unsafe extern "C" fn add_recipe(result: *const c_char, cnt: i32, ing: *const c_char, shapeless: bool) {
        let r = std::ffi::CStr::from_ptr(result).to_string_lossy().to_string();
        let g = std::ffi::CStr::from_ptr(ing).to_string_lossy().to_string();
        log(format!("add_recipe({r}x{cnt} <- [{g}] shapeless={shapeless})"));
    }
    #[no_mangle]
    pub unsafe extern "C" fn open_chest_ui(title: *const c_char, rows: i32, cb: i32) -> i32 {
        let t = std::ffi::CStr::from_ptr(title).to_string_lossy().to_string();
        log(format!("open_chest_ui(\"{t}\",rows={rows},cb={cb})"));
        1
    }
    #[no_mangle]
    pub unsafe extern "C" fn show_hud_message(msg: *const c_char, pos: i32) {
        let m = std::ffi::CStr::from_ptr(msg).to_string_lossy().to_string();
        log(format!("hud[{pos}]: {m}"));
    }
    #[no_mangle]
    pub unsafe extern "C" fn get_player_id() -> i32 { 0 }
    #[no_mangle]
    pub unsafe extern "C" fn get_targeted_block(_d: f32) -> i32 { -1 }
}

#[cfg(feature = "stub")]
pub use stubs::*;

/// Безопасная обёртка: Rust-строка → CString для FFI.
pub fn c(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s).unwrap_or_else(|_| std::ffi::CString::new("mod:error").unwrap())
}
