# Releasing

A release is a signed APK on the GitHub Releases page, built by `.github/workflows/release.yml` when a tag `vX.Y.Z`
is pushed. The version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `crates/core/Cargo.toml` and
`package.json` must match the tag (the workflow checks the first one).

## The signing key

Android installs an update only if it is signed with the same key as the installed app. **Lose the key and nobody
can update; they have to uninstall and reinstall.** Create it once and keep a backup somewhere safe, outside the
repository:

```sh
keytool -genkeypair -v -keystore release.jks -alias dtw -keyalg RSA -keysize 4096 -validity 36500
```

Never commit the keystore (`*.jks` and `*.keystore` are in `.gitignore`) and never put its passwords in a file in the
repository.

## Building a release locally

The Gradle project reads the key from environment variables, so nothing about it is stored in the repository:

```sh
export DTW_KEYSTORE_FILE=/absolute/path/to/release.jks
export DTW_KEYSTORE_PASSWORD=...
export DTW_KEY_ALIAS=dtw
export DTW_KEY_PASSWORD=...
./node_modules/.bin/tauri android build --apk --target aarch64 --features sqlite-bundled
```

The APK is `src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`. Check it:

```sh
$ANDROID_HOME/build-tools/36.0.0/apksigner verify --print-certs app-universal-release.apk
sha256sum app-universal-release.apk
```

Without `DTW_KEYSTORE_FILE` the release build is **unsigned** and cannot be installed.

## Releasing from CI

Add four repository secrets (Settings → Secrets and variables → Actions):

| Secret | Value |
| --- | --- |
| `DTW_KEYSTORE_BASE64` | `base64 -w0 release.jks` |
| `DTW_KEYSTORE_PASSWORD` | the keystore password |
| `DTW_KEY_ALIAS` | the alias (`dtw` above) |
| `DTW_KEY_PASSWORD` | the key password |

Then:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The workflow builds, verifies the signature, attaches the APK and its SHA-256 file, and generates release notes from
the commits.

## Checklist

- [ ] CI is green on the commit being tagged.
- [ ] Version bumped everywhere and matches the tag.
- [ ] `docs/OPEN-QUESTIONS.md` has nothing that blocks the release.
- [ ] The APK was installed on a real phone and the wallpaper changed.
