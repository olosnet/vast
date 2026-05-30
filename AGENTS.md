# AGENTS.md

## Session Rules
- Load and use `caveman` skill for all repo work.
- Use `caveman-compress` when compressing natural-language memory/instruction files.

## Repo Shape
- Rust edition 2024 workspace. Root `Cargo.toml` lists only `vast`, but `cargo metadata --no-deps` resolves both `vast` and `lvast`; use workspace commands when work spans crates.
- `vast/src/main.rs` is current binary entrypoint. It is interactive fake-camera FITS capture tool, not old SDK-version smoke test.
- `lvast/` is main library crate; `lvast/src/lib.rs` exposes `algos`, `base`, `cameras`, `drivers`, `imageformats`, `mounts`, `platesolvers`, and `types`.
- SVB bindgen output is `lvast/src/drivers/bindings/svb/driver.rs`; avoid hand edits unless intentionally patching generated code.
- `external/svb/` contains vendored SVB SDK header and per-arch native libs. `lvast/build.rs` maps target arch to `external/svb/lib/{x64,x86,armv8,armv7,armv6}`.

## Native Build Gotchas
- `lvast/build.rs` links static `SVBCameraSDK` plus dynamic `stdc++`, `usb-1.0`, `pthread`, and `m`; host needs C++ toolchain, libclang/bindgen support, and `libusb-1.0` linkable.
- Bindings regenerate during build from `external/svb/include/SVBCameraSDK.h` into `lvast/src/drivers/bindings/svb/driver.rs`.
- Unsupported Rust target arch panics in `build.rs`; supported mappings: `x86_64 -> x64`, `x86 -> x86`, `aarch64 -> armv8`, `armv7 -> armv7`, `arm -> armv6`.

## Commands
- Check compile: `cargo check --workspace`.
- Run all tests: `cargo test --workspace`.
- Focus fake-camera work: `cargo test -p lvast fake_camera -- --nocapture`.
- Compile tests only: `cargo test --workspace --no-run`.
- Strict lint: `cargo clippy --workspace -- -D warnings`.
- Run binary: `cargo run -p vast` launches interactive fake-camera capture prompts and writes FITS output.
- Format check: `cargo fmt --all -- --check` currently reports unrelated rustfmt diffs across repo; avoid repo-wide `cargo fmt` unless user wants formatting churn.
- Update vendored SVBONY/SVB SDK files: `scripts/update-svbony-drivers.sh [--no-check] <sdk-dir|sdk-archive|sdk-url>`; it backs up `external/svb` and runs `cargo check --workspace` unless skipped.

## Git / Generated Files
- `.gitignore` ignores `/target/`, `/gen/schemas`, and `Cargo.lock`, but this repo currently has `Cargo.lock` present. Do not delete it unless asked.
- `.agents/` and `skills-lock.json` may be untracked local OpenCode skill state; do not remove or commit unless user asks.
