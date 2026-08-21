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

### Linux-hosted Windows verification

Linux developers can supplement the native Windows CI gate by checking,
running blocking Clippy, and building the complete workspace for
`x86_64-pc-windows-msvc`. Install the exact approved tool and prerequisites:

```console
cargo install --locked cargo-xwin --version 0.23.1
rustup component add llvm-tools
rustup target add x86_64-pc-windows-msvc
```

Clang/LLVM is also required. On first use, cargo-xwin downloads and caches the
Microsoft SDK and CRT; using them accepts Microsoft's applicable license terms.
Run the supplementary gate with:

```console
cargo make verify-xwin
```

This is Linux-hosted compile, check, and Clippy evidence only. It does not run
Windows executables and does not use Wine. Native Windows CI and deliberate
real-window validation on Windows remain separate, stronger requirements.

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
cargo make verify          # comprehensive Linux gate, including xwin and advisory quality
cargo make verify-native   # format, lint, tests, build, docs, boundaries
cargo make verify-xwin     # Linux-hosted Windows check, Clippy, and build
cargo make quality         # non-blocking advisory maintainability checks
cargo make msrv            # separate Rust 1.95.0 check and test gate
cargo make policy          # advisory, license, source, and dependency policy
cargo make workflow-policy # actionlint and pedantic Zizmor
```

Focused tasks such as `format`, `check`, `clippy`, `test`, `doctest`, `build`,
`docs`, `cli-boundary`, `gui-scaffold`, and `coverage` are available for
iteration. `verify` includes `quality`, which reports additional Clippy
maintainability findings without making those findings blocking; operational
failures still fail the task. Hosted native CI runs the blocking gates through
`verify-native`; the separate Advisory Quality workflow reports maintainability
findings across Linux x86_64, both macOS architectures, and Windows x86_64.
Coverage produces `target/llvm-cov/coverage.lcov`; Stage 0 does not set an
arbitrary percentage threshold.

`cargo make verify` is Linux-hosted because it includes `verify-xwin` in
addition to the portable native gate and the pedantic Zizmor workflow audit.
Zizmor enables
online audits automatically when `ZIZMOR_GITHUB_TOKEN`, `GH_TOKEN`, or
`GITHUB_TOKEN` is available. Without one of those variables, it falls back to
offline auditing, which skips online-only checks and therefore does not provide
full parity with the hosted online Zizmor workflow.

For local development, `ZIZMOR_GITHUB_TOKEN` is the preferred project-specific
variable. Use a dedicated, least-privilege GitHub token with only the
permissions required for read-only auditing. Never commit tokens to repository
files.

## Dependency and license policy

Direct third-party dependencies are exactly pinned, `Cargo.lock` is committed,
and cargo-deny enforces the reviewed license and source policy across the Linux,
macOS, and Windows dependency branches. GitHub Actions are pinned to full commit
SHAs and tracked with Renovate for reviewed updates.

Droll is licensed under the [MIT License](LICENSE).
