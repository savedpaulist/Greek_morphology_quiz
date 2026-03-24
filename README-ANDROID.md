# Android Release

This project can produce a signed Android APK and AAB through the helper script in `scripts/android-release.zsh`.

## One-time setup

Create the signing keystore:

```zsh
./scripts/android-release.zsh init-keystore
```

By default, the keystore is stored at `~/.android/morph-app-upload-keystore.jks` with alias `morphapp`.

If you want a different path or alias, set them before running the script:

```zsh
export MORPH_APP_KEYSTORE_PATH="$HOME/.android/my-release-key.jks"
export MORPH_APP_KEY_ALIAS="myalias"
./scripts/android-release.zsh init-keystore
```

## Build signed APK

```zsh
./scripts/android-release.zsh apk
```

Result:

`target/dx/morph_app/release/android/app/app/build/outputs/apk/release/app-release-signed.apk`

## Build signed AAB

```zsh
./scripts/android-release.zsh aab
```

Result:

`target/dx/morph_app/release/android/app/app/build/outputs/bundle/release/app-release-signed.aab`

## Build both

```zsh
./scripts/android-release.zsh all
```

## What the script does

1. Exports Android SDK, NDK, and Java paths.
2. Runs `dx build --android --release`.
3. Runs the generated Gradle release task.
4. Signs the APK with `apksigner`.
5. Signs the AAB with `jarsigner`.

## Notes

- The script prompts for the keystore password when signing.
- If `ANDROID_NDK_HOME` is not already set, the script picks the newest NDK inside the local Android SDK.
- The generated Android Gradle project lives under `target/dx/...`, so the script recreates and signs artifacts from there automatically.