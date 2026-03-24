#!/usr/bin/env zsh

set -euo pipefail

SCRIPT_DIR=${0:A:h}
PROJECT_ROOT=${SCRIPT_DIR:h}
ANDROID_PROJECT_DIR="$PROJECT_ROOT/target/dx/morph_app/release/android/app"
DEFAULT_SDK_ROOT="$HOME/Library/Android/sdk"
DEFAULT_JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"

ACTION=${1:-help}

ANDROID_SDK_ROOT=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$DEFAULT_SDK_ROOT}}
ANDROID_HOME=${ANDROID_HOME:-$ANDROID_SDK_ROOT}
JAVA_HOME=${JAVA_HOME:-$DEFAULT_JAVA_HOME}

KEYSTORE_PATH=${MORPH_APP_KEYSTORE_PATH:-$HOME/.android/morph-app-upload-keystore.jks}
KEY_ALIAS=${MORPH_APP_KEY_ALIAS:-morphapp}
KEYTOOL_BIN=${MORPH_APP_KEYTOOL:-$JAVA_HOME/bin/keytool}
JARSIGNER_BIN=${MORPH_APP_JARSIGNER:-$JAVA_HOME/bin/jarsigner}

export ANDROID_SDK_ROOT
export ANDROID_HOME
export JAVA_HOME

print_usage() {
  cat <<'EOF'
Usage:
  ./scripts/android-release.zsh init-keystore
  ./scripts/android-release.zsh apk
  ./scripts/android-release.zsh aab
  ./scripts/android-release.zsh all

Commands:
  init-keystore  Create the signing keystore in ~/.android
  apk            Build and sign the release APK
  aab            Build and sign the release Android App Bundle
  all            Build and sign both APK and AAB

Optional environment variables:
  MORPH_APP_KEYSTORE_PATH  Override keystore path
  MORPH_APP_KEY_ALIAS      Override key alias (default: morphapp)
  ANDROID_SDK_ROOT         Override Android SDK root
  ANDROID_NDK_HOME         Override Android NDK root
  JAVA_HOME                Override JDK location

Outputs:
  target/dx/morph_app/release/android/app/app/build/outputs/apk/release/app-release-signed.apk
  target/dx/morph_app/release/android/app/app/build/outputs/bundle/release/app-release-signed.aab
EOF
}

fail() {
  echo "error: $1" >&2
  exit 1
}

require_file() {
  local path=$1
  [[ -f "$path" ]] || fail "missing file: $path"
}

require_dir() {
  local path=$1
  [[ -d "$path" ]] || fail "missing directory: $path"
}

find_latest_build_tools() {
  local latest
  latest=$(find "$ANDROID_SDK_ROOT/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)
  [[ -n "$latest" ]] || fail "no Android build-tools found under $ANDROID_SDK_ROOT/build-tools"
  echo "$latest"
}

detect_ndk_home() {
  if [[ -n "${ANDROID_NDK_HOME:-}" && -d "${ANDROID_NDK_HOME}" ]]; then
    echo "$ANDROID_NDK_HOME"
    return
  fi

  local detected
  detected=$(find "$ANDROID_SDK_ROOT/ndk" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1)
  [[ -n "$detected" ]] || fail "ANDROID_NDK_HOME is not set and no NDK was found under $ANDROID_SDK_ROOT/ndk"
  echo "$detected"
}

ensure_prerequisites() {
  require_dir "$PROJECT_ROOT"
  require_dir "$ANDROID_SDK_ROOT"
  require_file "$KEYTOOL_BIN"
  require_file "$JARSIGNER_BIN"

  export ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-$(detect_ndk_home)}
  require_dir "$ANDROID_NDK_HOME"

  BUILD_TOOLS_DIR=$(find_latest_build_tools)
  ZIPALIGN_BIN=${MORPH_APP_ZIPALIGN:-$BUILD_TOOLS_DIR/zipalign}
  APKSIGNER_BIN=${MORPH_APP_APKSIGNER:-$BUILD_TOOLS_DIR/apksigner}

  require_file "$ZIPALIGN_BIN"
  require_file "$APKSIGNER_BIN"
  command -v dx >/dev/null 2>&1 || fail "dx is not installed or not in PATH"
}

run_dx_android_release() {
  echo "> cargo clean --target-dir target/dx"
  (
    cd "$PROJECT_ROOT"
    cargo clean --target-dir target/dx
  )

  echo "> dx build --android --release"
  (
    cd "$PROJECT_ROOT"
    dx build --android --release
  )
}

ensure_android_project() {
  require_dir "$ANDROID_PROJECT_DIR"
  require_file "$ANDROID_PROJECT_DIR/gradlew"
}

gradle_task() {
  local task=$1
  echo "> ./gradlew $task"
  (
    cd "$ANDROID_PROJECT_DIR"
    ./gradlew "$task"
  )
}

replace_android_icons() {
  echo "> replacing android launcher icons"
  local res_dir="$ANDROID_PROJECT_DIR/app/src/main/res"
  local icon_root="$PROJECT_ROOT/assets/icons/android"

  if [[ -d "$icon_root" && -d "$res_dir" ]]; then
    local densities=(mdpi hdpi xhdpi xxhdpi xxxhdpi)
    local density=

    for density in $densities; do
      local source_dir="$icon_root/mipmap-$density"
      local target_dir="$res_dir/mipmap-$density"

      if [[ -f "$source_dir/ic_launcher.png" ]]; then
        cp "$source_dir/ic_launcher.png" "$target_dir/ic_launcher.png"
      fi

      if [[ -f "$source_dir/ic_launcher_round.png" ]]; then
        cp "$source_dir/ic_launcher_round.png" "$target_dir/ic_launcher_round.png"
      fi
    done

    # Clean up default Dioxus-generated assets if present.
    rm -f "$res_dir"/mipmap-*/ic_launcher.webp
  else
    echo "Warning: Android icon directories not found, skipping icon replacement."
  fi
}

create_keystore() {
  ensure_prerequisites

  if [[ -f "$KEYSTORE_PATH" ]]; then
    echo "Keystore already exists: $KEYSTORE_PATH"
    return
  fi

  mkdir -p "${KEYSTORE_PATH:h}"
  echo "> creating keystore: $KEYSTORE_PATH"
  "$KEYTOOL_BIN" \
    -genkeypair \
    -v \
    -keystore "$KEYSTORE_PATH" \
    -alias "$KEY_ALIAS" \
    -keyalg RSA \
    -keysize 2048 \
    -validity 10000

  echo "Keystore created: $KEYSTORE_PATH"
}

ensure_keystore_exists() {
  [[ -f "$KEYSTORE_PATH" ]] || fail "keystore not found: $KEYSTORE_PATH. Run './scripts/android-release.zsh init-keystore' first"
}

sign_apk() {
  local unsigned_apk="$ANDROID_PROJECT_DIR/app/build/outputs/apk/release/app-release-unsigned.apk"
  local aligned_apk="$ANDROID_PROJECT_DIR/app/build/outputs/apk/release/app-release-aligned.apk"
  local signed_apk="$ANDROID_PROJECT_DIR/app/build/outputs/apk/release/app-release-signed.apk"

  require_file "$unsigned_apk"

  echo "> zipalign release APK"
  "$ZIPALIGN_BIN" -p -f 4 "$unsigned_apk" "$aligned_apk"

  echo "> apksigner sign release APK"
  "$APKSIGNER_BIN" sign \
    --ks "$KEYSTORE_PATH" \
    --ks-key-alias "$KEY_ALIAS" \
    --out "$signed_apk" \
    "$aligned_apk"

  echo "> apksigner verify release APK"
  "$APKSIGNER_BIN" verify -v "$signed_apk"
  echo "Signed APK: $signed_apk"
}

sign_aab() {
  local unsigned_aab="$ANDROID_PROJECT_DIR/app/build/outputs/bundle/release/app-release.aab"
  local signed_aab="$ANDROID_PROJECT_DIR/app/build/outputs/bundle/release/app-release-signed.aab"

  require_file "$unsigned_aab"
  rm -f "$signed_aab"

  echo "> jarsigner sign release AAB"
  "$JARSIGNER_BIN" \
    -keystore "$KEYSTORE_PATH" \
    -signedjar "$signed_aab" \
    "$unsigned_aab" \
    "$KEY_ALIAS"

  echo "> jarsigner verify release AAB"
  "$JARSIGNER_BIN" -verify "$signed_aab"
  echo "Signed AAB: $signed_aab"
}

build_signed_apk() {
  ensure_prerequisites
  ensure_keystore_exists
  run_dx_android_release
  ensure_android_project
  replace_android_icons
  gradle_task assembleRelease
  sign_apk
}

build_signed_aab() {
  ensure_prerequisites
  ensure_keystore_exists
  run_dx_android_release
  ensure_android_project
  replace_android_icons
  gradle_task bundleRelease
  sign_aab
}

build_all() {
  ensure_prerequisites
  ensure_keystore_exists
  run_dx_android_release
  ensure_android_project
  replace_android_icons
  gradle_task assembleRelease
  gradle_task bundleRelease
  sign_apk
  sign_aab
}

case "$ACTION" in
  init-keystore)
    create_keystore
    ;;
  apk)
    build_signed_apk
    ;;
  aab)
    build_signed_aab
    ;;
  all)
    build_all
    ;;
  help|-h|--help)
    print_usage
    ;;
  *)
    print_usage
    fail "unknown command: $ACTION"
    ;;
esac