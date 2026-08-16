# Droll

Droll is the foundation of a cross-platform RPG dice roller with a lightweight
command-line interface and a native 3D graphical application. The repository is
currently at **Stage 0**: its workspace, architectural boundaries, and quality
gates exist, but dice rolling and finished CLI/GUI behavior do not.

## Architecture

The Rust 2024 workspace contains three packages:

- `droll-core`: a GUI-independent domain library with no external dependencies;
- `droll-cli`: the lightweight package that emits the `droll` binary and depends
  only on `droll-core` at runtime; and
- `droll-gui`: the package that emits `droll-gui` and exclusively owns the Bevy
  and Avian graphical stack.

The Stage 0 `droll` binary supports conventional `--help` and `--version`
output. Roll parsing, evaluation, and no-argument GUI dispatch are later-stage
work. The `droll-gui` binary constructs the minimal Bevy/Avian application; it
does not yet contain layout, interaction, dice meshes, or roll physics.

## Supported platforms

Droll targets native desktop builds on:

- Linux x86_64 (`ubuntu-24.04` in CI);
- macOS Apple Silicon (`macos-15` in CI);
- macOS Intel (`macos-15-intel` in CI); and
- Windows x86_64 with MSVC (`windows-2025` in CI).

The development toolchain is Rust 1.97.1. The declared and separately checked
minimum supported Rust version is 1.95.0.

### Native prerequisites

On Ubuntu 24.04, install the native libraries required by the selected X11 and
Wayland backends:

```console
sudo apt-get install g++ pkg-config libx11-dev libwayland-dev \
  libxkbcommon-dev libxkbcommon-x11-0
```

macOS requires the Xcode command-line tools. Windows requires the MSVC C++ build
tools and Windows SDK. No Homebrew, MinGW, Vulkan SDK, or cross-compilation
toolchain is required by the Stage 0 baseline.

## Development

The pinned `rust-toolchain.toml` selects Rust 1.97.1 with rustfmt and Clippy.
Install the exact local tools used by the checked-in tasks:

- cargo-make 0.37.24;
- cargo-nextest 0.9.143;
- cargo-llvm-cov 0.8.7;
- cargo-audit 0.22.2;
- cargo-deny 0.20.2;
- actionlint 1.7.12; and
- Zizmor 1.29.0.

Build the complete native workspace and both binaries with:

```console
cargo build --workspace --all-targets --all-features --locked
```

`cargo-make` is the canonical task runner:

```console
cargo make verify          # comprehensive Rust 1.97.1 local gate
cargo make verify-native   # format, lint, tests, build, docs, boundaries
cargo make msrv            # separate Rust 1.95.0 check and test gate
cargo make policy          # advisory, license, source, and dependency policy
cargo make workflow-policy # actionlint and pedantic Zizmor
```

Focused tasks such as `format`, `check`, `clippy`, `test`, `doctest`, `build`,
`docs`, `cli-boundary`, `gui-scaffold`, and `coverage` are available for
iteration. Coverage produces `target/llvm-cov/coverage.lcov`; Stage 0 does not
set an arbitrary percentage threshold.

## Dependency and license policy

Direct third-party dependencies are exactly pinned, `Cargo.lock` is committed,
and cargo-deny enforces the reviewed license and source policy across the Linux,
macOS, and Windows dependency branches. GitHub Actions are pinned to full commit
SHAs and tracked with Renovate for reviewed updates.

Droll is licensed under the [MIT License](LICENSE).
