//! IronChest-Horizon — аналог мода IronChest для Minecraft Horizon (Mod Studio).
//!
//! Точка входа: `extern "C" fn modentry()` — её вызывает рантайм Horizon после
//! загрузки нативной библиотеки мода (.hmod).
//!
//! Возможности:
//!  * Железный / золотой / алмазный / обсидиановый / кристальный сундуки
//!    (54 / 81 / 108 / 180 / 216 слотов — как в IronChest + CraftStorages);
//!  * крафты-апгрейды по цепочке сундук → железо → золото → алмаз → обсидиан → кристалл;
//!  * персистентное хранение содержимого каждого сундука между запусками мира;
//!  * при разрушении сундука всё его содержимое выпадает игроку.

mod chest;
mod horizon;

use chest::{BlockPos, ChestRecord, ChestStorage, ChestType, ItemStack};
use horizon::c;
use std::sync::{Mutex, OnceLock};

static STORAGE: OnceLock<Mutex<ChestStorage>> = OnceLock::new();
/// Путь к файлу сохранений задаёт рантайм через `set_save_path` (в .hmod это
/// `data/contents.dat` внутри папки мода на устройстве).
static SAVE_PATH: OnceLock<Mutex<String>> = OnceLock::new();

fn save_path() -> String {
    SAVE_PATH
        .get_or_init(|| Mutex::new(String::from("data/contents.dat")))
        .lock()
        .map(|p| p.clone())
        .unwrap_or_else(|_| String::from("data/contents.dat"))
}

fn storage() -> &'static Mutex<ChestStorage> {
    STORAGE.get_or_init(|| {
        let st = std::fs::read(save_path())
            .ok()
            .and_then(|b| ChestStorage::from_bytes(&b))
            .unwrap_or_default();
        Mutex::new(st)
    })
}

fn save() {
    if let Ok(st) = storage().lock() {
        let bytes = st.to_bytes();
        if let Some(dir) = std::path::Path::new(&save_path()).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(save_path(), bytes);
    }
}

// ======================= Точка входа =======================

/// Экспорт, который вызывает Mod Studio при загрузке мода.
#[no_mangle]
pub extern "C" fn modentry() {
    unsafe {
        // 1. Регистрируем предметы-компоненты новых блоков.
        for t in ChestType::ALL.iter().skip(1) {
            let name = c(t.item_name());
            let title = c(t.display_name_ru());
            let icon = c(&format!("gui/{}", t.item_name().replace("mod:", "")));
            horizon::add_item(name.as_ptr(), title.as_ptr(), icon.as_ptr(), 1);
            let block = c(&t.item_name().replace("mod:", "mod_"));
            horizon::create_item_with_component(name.as_ptr(), block.as_ptr(), icon.as_ptr());
        }
        // Предмет-осколок для кристального сундука.
        horizon::add_item(
            c("mod:crystal_shard").as_ptr(),
            c("Кристаллический осколок").as_ptr(),
            c("gui/crystal_shard").as_ptr(),
            16,
        );

        // 2. Рецепты (shapeless, как в IronChest: старый сундук + материалы).
        for (result, ings) in chest::recipes() {
            let joined = ings
                .iter()
                .map(|(n, cnt)| format!("{n}[{cnt}]"))
                .collect::<Vec<_>>()
                .join(";");
            horizon::add_recipe(c(result).as_ptr(), 1, c(&joined).as_ptr(), true);
        }

        // 3. Подхватываем ранее сохранённое содержимое сундуков.
        let _ = storage();

        horizon::show_hud_message(c("IronChest-Horizon загружен (Rust)").as_ptr(), 3);
    }
}

/// Рантайм может вызвать это до `modentry`, чтобы указать файл сохранений.
/// Экспортируется как `set_save_path`, без конфликта с приватной функцией.
#[no_mangle]
#[export_name = "set_save_path"]
pub extern "C" fn horizon_set_save_path(path: *const std::ffi::c_char) {
    if path.is_null() {
        return;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy().to_string();
    *SAVE_PATH.get_or_init(|| Mutex::new(String::new())).lock().unwrap() = s;
}

// ======================= События мира =======================

/// Вызывается рантаймом при постановке блока игроком.
/// Возвращает 1, если событие обработано модом.
#[no_mangle]
pub extern "C" fn on_block_place(x: i32, y: i32, z: i32, item_name: *const std::ffi::c_char) -> i32 {
    if item_name.is_null() {
        return 0;
    }
    let name = unsafe { std::ffi::CStr::from_ptr(item_name) }.to_string_lossy().to_string();
    let pos = BlockPos::new(current_dim(), x, y, z);

    // Ставим обычный сундук предметом-сундуком или апгрейдим стоящий —
    // см. ветвление ниже.
    let ct = match ChestType::from_item_name(&name) {
        Some(ct) => ct,
        None => {
            // Не сундук? Возможно, это материал апгрейда по уже стоящему сундуку
            // (железный слиток ПКМ по обычному сундуку — быстрый upgrade in-world).
            let mut st = storage().lock().unwrap();
            if st.get(pos).is_some() && st.upgrade_at(pos, &name).is_some() {
                drop(st);
                let t = storage().lock().unwrap().get(pos).unwrap().chest_type;
                unsafe {
                    horizon::show_hud_message(
                        c(&format!("Сундук улучшен: {}", t.display_name_ru())).as_ptr(),
                        3,
                    );
                }
                save();
                return 1;
            }
            return 0;
        }
    };

    {
        let mut st = storage().lock().unwrap();
        // Если на этом месте уже стоит сундук — повышаем уровень (крафт прямо в мире).
        if st.get(pos).is_some() {
            if let Some(new_t) = st.upgrade_at(pos, chest::upgrade_material_of(ct).unwrap_or(&name)) {
                drop(st);
                unsafe {
                    horizon::show_hud_message(
                        c(&format!("Сундук улучшен: {}", new_t.display_name_ru())).as_ptr(),
                        3,
                    );
                }
                save();
                return 1;
            }
        }
        st.place(pos, ct);
    }
    save();
    1
}

/// Вызывается рантаймом при разрушении блока. Возвращает 1, если блокировать
/// стандартный дроп (мод сам выдаст содержимое).
#[no_mangle]
pub extern "C" fn on_block_break(x: i32, y: i32, z: i32) -> i32 {
    let pos = BlockPos::new(current_dim(), x, y, z);
    let rec = {
        let mut st = storage().lock().unwrap();
        st.remove(pos)
    };
    let Some(rec) = rec else { return 0 };

    let player = unsafe { horizon::get_player_id() };
    // Содержимое — игроку (остаток — в мир).
    for slot in rec.items.iter().flatten() {
        give(player, slot, x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
    }
    // Сам блок-сундук тоже должен выпасть.
    unsafe {
        horizon::spawn_item_to_player(player, c(chest_name_of(&rec)).as_ptr(), 1, 0);
    }
    save();
    1
}

fn chest_name_of(rec: &ChestRecord) -> &'static str {
    rec.chest_type.item_name()
}

/// Вызывается при открытии (тапе по) сундука: возвращает id GUI-колбэка или -1.
#[no_mangle]
pub extern "C" fn on_chest_open(x: i32, y: i32, z: i32) -> i32 {
    let pos = BlockPos::new(current_dim(), x, y, z);
    let st = storage().lock().unwrap();
    let Some(rec) = st.get(pos) else { return -1 };
    let rows = (rec.chest_type.slots() / 9) as i32;
    let title = c(rec.chest_type.display_name_ru());
    drop(st);
    unsafe { horizon::open_chest_ui(title.as_ptr(), rows, 1) }
}

/// Колбэк закрытия GUI сундука: рантайм передаёт содержимое плоскими массивами
/// (`counts[i] > 0` ⇒ предмет `names[i]`). Мод сохраняет его в своё хранилище.
#[no_mangle]
pub unsafe extern "C" fn on_chest_close(
    x: i32,
    y: i32,
    z: i32,
    names: *const *const std::ffi::c_char,
    counts: *const i32,
    datas: *const i32,
    len: i32,
) {
    if names.is_null() || counts.is_null() || datas.is_null() || len <= 0 {
        return;
    }
    let pos = BlockPos::new(current_dim(), x, y, z);
    {
        let mut st = storage().lock().unwrap();
        if let Some(rec) = st.get_mut(pos) {
            for s in rec.items.iter_mut() {
                *s = None;
            }
            for i in 0..len as usize {
                let cnt = *counts.add(i);
                if cnt <= 0 {
                    continue;
                }
                let nm = std::ffi::CStr::from_ptr(*names.add(i)).to_string_lossy().to_string();
                let data = *datas.add(i);
                rec.insert(&ItemStack::new(nm, cnt, data), 64);
            }
        }
    }
    save();
}

fn give(player: i32, stack: &ItemStack, fx: f32, fy: f32, fz: f32) {
    unsafe {
        let p = c(&stack.name);
        // Сначала пробуем выдать игроку; не удалось — спавним предмет в мир.
        let spawned = horizon::spawn_item_to_player(player, p.as_ptr(), stack.count, stack.data);
        if spawned.is_null() {
            horizon::spawn_item(fx, fy, fz, p.as_ptr(), stack.count, stack.data);
        }
    }
}

fn current_dim() -> i32 {
    // Horizon: 0 — обычный мир, 1 — незер. Рантайм сообщает активное измерение
    // через глобальную переменную окружения скрипта; по умолчанию 0.
    static DIM: OnceLock<i32> = OnceLock::new();
    *DIM.get_or_init(|| std::env::var("HORIZON_DIM").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}

// ======================= Автотесты ядра =======================

#[cfg(all(test, feature = "stub"))]
mod tests {
    use super::*;

    #[test]
    fn full_lifecycle() {
        unsafe {
            modentry();

            // Ставим железный сундук.
            let r = on_block_place(10, 64, 10, c("mod:iron_chest").as_ptr());
            assert_eq!(r, 1);
            let pos = BlockPos::new(0, 10, 64, 10);
            assert_eq!(storage().lock().unwrap().get(pos).unwrap().chest_type, ChestType::Iron);

            // Апгрейд materials: ставим золотой слиток поверх — получаем золотой сундук.
            let r = on_block_place(10, 64, 10, c("gold_ingot").as_ptr());
            assert_eq!(r, 1);
            assert_eq!(storage().lock().unwrap().get(pos).unwrap().chest_type, ChestType::Gold);

            // Открываем GUI — 9 рядов.
            assert!(on_chest_open(10, 64, 10) >= 0);

            // Закрываем с содержимым: 64 камня.
            let mut stone_raw = std::mem::ManuallyDrop::new(c("stone"));
            let owned: [*const std::ffi::c_char; 1] = [stone_raw.as_ptr() as *const _];
            let counts = [64i32];
            let datas = [0i32];
            on_chest_close(10, 64, 10, owned.as_ptr(), counts.as_ptr(), datas.as_ptr(), 1);
            let rec = storage().lock().unwrap().get(pos).cloned().unwrap();
            assert_eq!(rec.total_items(), 1);
            assert_eq!(rec.items[0].as_ref().unwrap().count, 64);

            // Разрушаем — контент возвращается игроку, запись удаляется.
            assert_eq!(on_block_break(10, 64, 10), 1);
            assert!(storage().lock().unwrap().get(pos).is_none());

            let log = horizon::LOG.lock().unwrap().join("\n");
            assert!(log.contains("spawn_item_to_player(p0,stonex64:0)"), "{log}");
            assert!(log.contains("spawn_item_to_player(p0,mod:gold_chestx1:0)"), "{log}");
            assert!(log.contains("add_recipe(mod:iron_chest"), "{log}");
            assert!(log.contains("Сундук улучшен"), "{log}");
        }
    }

    #[test]
    fn vanilla_chest_not_tracked() {
        // Постановка неизвестного предмета не обрабатывается модом.
        assert_eq!(on_block_place(1, 1, 1, c("dirt").as_ptr()), 0);
        // …а разрушение нетрекируемого блока возвращает 0 (стандартный дроп).
        assert_eq!(on_block_break(1, 1, 1), 0);
    }
}
