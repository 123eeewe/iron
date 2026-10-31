#!/usr/bin/env bash
# build_hmod.sh — сборка мода IronChest-Horizon в формат .hmod
#
# Что делает скрипт:
#   1. Кросс-компилирует Rust-библиотеку под Android (arm64-v8a, по желанию armeabi-v7a)
#      через Android NDK. Без NDK выполняет host-сборку (только для проверки кода).
#   2. Формирует структуру мода: manifest.json, pack_icon.(png|json), lib/<abi>/libironchest.so,
#      data/contents.dat.
#   3. Упаковывает всё в ZIP-архив с расширением .hmod.
#
# Использование:
#   ./build_hmod.sh                 # release-сборка arm64 -> dist/ironchest_horizon.hmod
#   ABI=both ./build_hmod.sh        # arm64 + armeabi-v7a
#   ANDROID_NDK_HOME=/path/to/ndk ./build_hmod.sh
#   ./build_hmod.sh --host          # хостовая сборка без NDK (проверка компиляции)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE="$ROOT/ironchest-horizon"
DIST="$ROOT/dist"
STAGE="$DIST/stage"
NAME="ironchest_horizon"
ABI="${ABI:-arm64}"
TARGET_TRIPLE="aarch64-linux-android"
API_LEVEL="${API_LEVEL:-21}"

if [[ "${1:-}" == "--host" ]]; then
  TARGET_TRIPLE="$(rustc -vV | sed -n 's|^host: ||p')"
fi

echo "==> Целевой тулчейн: $TARGET_TRIPLE"

# --- 1. Компиляция -----------------------------------------------------------
export CARGO_TARGET_DIR="$CRATE/target"
if [[ "$TARGET_TRIPLE" != "$(rustc -vV | sed -n 's|^host: ||p')" ]]; then
  if [[ -z "${ANDROID_NDK_HOME:-}" || ! -d "$ANDROID_NDK_HOME" ]]; then
    echo "ОШИБКА: не задан ANDROID_NDK_HOME (нужен Android NDK для сборки под .hmod)." >&2
    echo "Установка: sdkmanager 'ndk;26.1.10909125'  ИЛИ скачайте с developer.android.com/ndk" >&2
    echo "Для проверки кода без NDK запустите: ./build_hmod.sh --host" >&2
    exit 1
  fi
  HOST_TAG="$(uname -s | tr 'A-Z' 'a-z')-x86_64"
  CC_NAME="${TARGET_TRIPLE}${API_LEVEL}-clang"
  export CC_${TARGET_TRIPLE//-/_}="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG/bin/$CC_NAME"
  export AR_${TARGET_TRIPLE//-/_}="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG/bin/llvm-ar"
  rustup target add "$TARGET_TRIPLE" >/dev/null 2>&1 || true
fi

cargo build --release --manifest-path "$CRATE/Cargo.toml" --target "$TARGET_TRIPLE"
SO="$CARGO_TARGET_DIR/$TARGET_TRIPLE/release/libironchest.so"
[[ -f "$SO" ]] || { echo "ОШИБКА: не найден $SO" >&2; exit 1; }

# Проверка обязательной точки входа modentry
if command -v nm >/dev/null 2>&1; then
  nm -D "$SO" 2>/dev/null | grep -q ' T modentry' \
    || echo "ПРЕДУПРЕЖДЕНИЕ: символ 'modentry' не найден в $SO (проверьте #[no_mangle] pub extern \"C\" fn modentry)" >&2
fi

# --- 2. Подготовка структуры мода --------------------------------------------
rm -rf "$STAGE"; mkdir -p "$STAGE/lib/arm64-v8a" "$STAGE/data"
cp "$SO" "$STAGE/lib/arm64-v8a/libironchest.so"

if [[ "$ABI" == "both" ]]; then
  V7="armv7-linux-androideabi"
  rustup target add "$V7" >/dev/null 2>&1 || true
  cargo build --release --manifest-path "$CRATE/Cargo.toml" --target "$V7"
  mkdir -p "$STAGE/lib/armeabi-v7a"
  cp "$CARGO_TARGET_DIR/$V7/release/libironchest.so" "$STAGE/lib/armeabi-v7a/libironchest.so"
fi

cp "$ROOT/manifest.json" "$STAGE/manifest.json"
# pack_icon: берём PNG если есть, иначе кладём шаблон JSON рядом
if [[ -f "$ROOT/pack_icon.png" ]]; then
  cp "$ROOT/pack_icon.png" "$STAGE/pack_icon.png"
else
  echo "ПРЕДУПРЕЖДЕНИЕ: pack_icon.png отсутствует — кладу шаблон pack_icon.json." >&2
  cp "$ROOT/pack_icon.json" "$STAGE/pack_icon.json"
fi
touch "$STAGE/data/contents.dat"   # пустой файл сохранений; создаётся рантаймом

# --- 3. Упаковка в .hmod (ZIP) --------------------------------------------------
mkdir -p "$DIST"
OUT="$DIST/$NAME.hmod"
rm -f "$OUT"
if command -v zip >/dev/null 2>&1; then
  (cd "$STAGE" && zip -X -r "$OUT" . >/dev/null)
elif command -v python3 >/dev/null 2>&1; then
  python3 - "$STAGE" "$OUT" <<'PY'
import os, sys, zipfile
stage, out = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for base, _, files in os.walk(stage):
        for f in files:
            p = os.path.join(base, f)
            z.write(p, os.path.relpath(p, stage))
print("Записано:", out)
PY
else
  echo "ОШИБКА: нужны zip или python3 для упаковки архива." >&2; exit 1
fi

echo "==> Готово: $OUT"
echo "==> Установка: импортируйте .hmod через Mod Studio Horizon (Импорт мода),"
echo "    либо скопируйте в папку модов игры на устройстве."
