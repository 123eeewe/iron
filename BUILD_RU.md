# Сборка IronChest-Horizon в формат `.hmod`

Инструкция для Windows, macOS и Linux. Формат `.hmod` — это ZIP-архив специальной
структуры, который Mod Studio Horizon импортирует как нативный мод (Rust/C++ `.so`).

## 1. Требования

| Компонент | Зачем | Как проверить |
|---|---|---|
| Rust ≥ 1.75 (`rustup`) | компиляция crate `ironchest` | `rustc -V` |
| Android NDK r26+ | кросс-компиляция под `aarch64-linux-android` | переменная `ANDROID_NDK_HOME` |
| Целевой тулчейн | сборка `.so` для ARM | `rustup target add aarch64-linux-android` |
| `zip` **или** `python3` | упаковка архива `.hmod` | `zip -v` / `python3 -V` |
| Minecraft Horizon + Mod Studio | импорт и запуск мода на устройстве | — |

Переменные окружения:

```bash
export ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/26.1.10909125"   # Linux/macOS
# Windows (PowerShell):
# $env:ANDROID_NDK_HOME = "C:\Android\Sdk\ndk\26.1.10909125"
```

## 2. Быстрая сборка

В корне репозитория:

```bash
chmod +x build_hmod.sh          # только на Linux/macOS
./build_hmod.sh                 # arm64-v8a -> dist/ironchest_horizon.hmod
ABI=both ./build_hmod.sh        # дополнительно armeabi-v7a
./build_hmod.sh --host          # проверка компиляции без NDK (не для .hmod!)
```

Результат — файл **`dist/ironchest_horizon.hmod`**.

### Windows без WSL

`build_hmod.sh` рассчитан на bash (Git Bash / WSL). В PowerShell можно выполнить
шаги вручную (см. раздел 4), либо запустить тот же скрипт через Git Bash:

```powershell
& "C:\Program Files\Git\bin\bash.exe" ./build_hmod.sh
```

## 3. Структура готового `.hmod`

```text
ironchest_horizon.hmod          (ZIP)
├── manifest.json               шаблон в репозитории — скопируйте и отредактируйте uuid
├── pack_icon.png               иконка 128x128 (PNG); шаблон описания — pack_icon.json
├── lib/
│   ├── arm64-v8a/libironchest.so
│   └── armeabi-v7a/libironchest.so   (если ABI=both)
└── data/contents.dat           файл сохранений сундуков (пустой до первой игры)
```

Ключевые поля `manifest.json`:

* `mod.entry_point` — **`modentry`**: именно этот символ рантайм Horizon вызывает
  после загрузки библиотеки. Он объявлен в `src/lib.rs` как
  `#[no_mangle] pub extern "C" fn modentry()`.
* `mod.library` — имя `.so` (`libironchest.so`).
* `uuid` модуля и хедера должны быть уникальны при каждом изменении версии.

## 4. Ручная сборка (по шагам)

```bash
cd ironchest-horizon
rustup target add aarch64-linux-android
cargo build --release --target aarch64-linux-android
# линковщик-обёртка NDK задаётся в .cargo/config.toml; при проблемах:
# export CC_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang"

mkdir -p stage/lib/arm64-v8a stage/data
cp target/aarch64-linux-android/release/libironchest.so stage/lib/arm64-v8a/
cp ../manifest.json stage/
cp ../pack_icon.png stage/ 2>/dev/null || cp ../pack_icon.json stage/
touch stage/data/contents.dat
(cd stage && zip -X -r ../dist/ironchest_horizon.hmod .)
```

Проверка точки входа:

```bash
nm -D stage/lib/arm64-v8a/libironchest.so | grep modentry
# должно быть: 00000000000xxxxx T modentry
```

## 5. Установка в игру

1. Откройте **Mod Studio Horizon** → «Импорт мода» → выберите `ironchest_horizon.hmod`.
2. Активируйте мод в настройках мира (требуется включённыйexperimental-режим Native mods).
3. На устройстве мод хранит сохранения в `data/contents.dat` внутри своей папки.

## 6. Типичные проблемы

| Симптом | Причина / решение |
|---|---|
| `error: linker 'aarch64-linux-android21-clang' not found` | Не задан `ANDROID_NDK_HOME` или неверный `HOST_TAG`; проверьте путь к NDK и `ar`/`linker` в `.cargo/config.toml` |
| `ПРЕДУПРЕЖДЕНИЕ: символ 'modentry' не найден` | Сломан `#[no_mangle] pub extern "C"` в `src/lib.rs`, либо сборка пошла не под Android-таргет |
| Игра не видит мод | Проверьте `manifest.json` (JSON должен быть валидным), переименуйте архив ровно в `*.hmod`, переустановите через Mod Studio |
| Ошибки крафта/GUI после обновления API | Соберите заново: `cargo clean --target aarch64-linux-android && ./build_hmod.sh` |
| Нет `zip` | Скрипт автоматически использует `python3` для упаковки |

## 7. Тесты ядра (без устройства)

```bash
cd ironchest-horizon
cargo test --features stub        # логика сундуков, сериализация, апгрейды
```

## 8. Файлы репозитория

| Файл | Назначение |
|---|---|
| `BUILD_RU.md` | эта инструкция |
| `build_hmod.sh` | скрипт сборки `.hmod` (NDK → ZIP) |
| `manifest.json` | шаблон манифеста мода — скопируйте в корень архива, замените uuid на свои |
| `pack_icon.json` | шаблон описания иконки/текстур; в сам архив кладите готовый `pack_icon.png` (128×128) |
| `ironchest-horizon/` | Rust-crate мода (`src/lib.rs`, точка входа `modentry`) |
| `dist/` | результат сборки (в `.gitignore`) |
