#!/usr/bin/env bash
set -euo pipefail

: "${RUNNER_TEMP:?Run this script in GitHub Actions}"
ci_root="$RUNNER_TEMP/cliary-ci"
export RUSTUP_HOME="$ci_root/rustup"
export CARGO_HOME="$ci_root/cargo"
export TMPDIR="$ci_root/tmp"
export XDG_CACHE_HOME="$ci_root/cache"
export ZIG_GLOBAL_CACHE_DIR="$ci_root/cache/zig"
export CARGO_ZIGBUILD_CACHE_DIR="$ci_root/cache/cargo-zigbuild"
export CLIARY_CONFIG_DIR="$ci_root/config"
export CLIARY_DATA_DIR="$ci_root/data"
export CLIARY_CACHE_DIR="$ci_root/cache/cliary"
export CARGO_BUILD_JOBS=2
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_PROFILE_RELEASE_STRIP=symbols
export PIP_NO_CACHE_DIR=1
export PYTHONDONTWRITEBYTECODE=1
mkdir -p "$TMPDIR" "$XDG_CACHE_HOME" dist .ci-reports
export PATH="$CARGO_HOME/bin:$PWD/.ci-venv/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.94.1
export CARGO_ZIGBUILD_PYTHON_PATH="$PWD/.ci-venv/bin/python"
echo "Runner: ${RUNNER_ENVIRONMENT:-unknown} / ${RUNNER_OS:-unknown} / ${RUNNER_ARCH:-unknown}"
free -m
df -h .

python3 -m venv .ci-venv
python -m pip install --disable-pip-version-check \
  PyYAML==6.0.2 jsonschema==4.23.0 tomli==2.2.1

rustup_url=https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
curl --fail --location --retry 3 "$rustup_url" -o "$TMPDIR/rustup-init"
curl --fail --location --retry 3 "$rustup_url.sha256" -o "$TMPDIR/rustup-init.sha256"
(cd "$TMPDIR"; sha256sum --check rustup-init.sha256)
chmod +x "$TMPDIR/rustup-init"
"$TMPDIR/rustup-init" -y --no-modify-path --profile minimal --default-toolchain "$RUSTUP_TOOLCHAIN"
rustup component add rustfmt --toolchain "$RUSTUP_TOOLCHAIN"
rm "$TMPDIR/rustup-init" "$TMPDIR/rustup-init.sha256"
rustc --version

python scripts/validate_catalog.py
python scripts/test_ci_package.py
cargo fmt --all -- --check
cargo run --locked -p cliary-catalog --bin cliary-catalog-build -- validate
cargo test --workspace --locked
bash -n crates/cliary-cli/shell/bash.sh
zsh -n crates/cliary-cli/shell/zsh.sh
fish -n crates/cliary-cli/shell/fish.fish

if [[ "$CLIARY_PACKAGE" != true ]]; then exit 0; fi
python -m pip install --disable-pip-version-check cargo-zigbuild==0.23.4 ziglang==0.16.0
cargo zigbuild --version
python -m ziglang version
# Catalog is bundled into the program; release assets contain executables only.
rm -rf target/debug
targets=(x86_64-unknown-linux-musl aarch64-unknown-linux-musl)
if [[ "$CLIARY_BUILD_MACOS" == true ]]; then
  mkdir -p .ci-sdk
  sdk_archive="$TMPDIR/MacOSX11.3.sdk.tar.xz"
  curl --fail --location --retry 3 \
    https://github.com/phracker/MacOSX-SDKs/releases/download/11.3/MacOSX11.3.sdk.tar.xz \
    -o "$sdk_archive"
  printf '%s  %s\n' cd4f08a75577145b8f05245a2975f7c81401d75e9535dcffbb879ee1deefcbf4 "$sdk_archive" | sha256sum --check
  tar -xJf "$sdk_archive" -C .ci-sdk
  rm "$sdk_archive"
  targets+=(x86_64-apple-darwin aarch64-apple-darwin)
fi

for target in "${targets[@]}"; do
  rustup target add "$target"
  if [[ "$target" == *apple-darwin ]]; then
    export SDKROOT="$PWD/.ci-sdk/MacOSX11.3.sdk"
    export MACOSX_DEPLOYMENT_TARGET=13.0
  else
    unset SDKROOT MACOSX_DEPLOYMENT_TARGET
  fi
  started=$SECONDS
  cargo zigbuild --locked --release --target "$target" -p cliary-cli
  printf '%s build seconds: %s\n' "$target" "$((SECONDS - started))"
  if [[ "$target" == *linux-musl ]]; then
    if readelf -lW "target/$target/release/cliary" | grep -q INTERP; then
      echo 'Unexpected ELF interpreter' >&2; exit 1
    fi
    if readelf -dW "target/$target/release/cliary" | grep -q NEEDED; then
      echo 'Unexpected shared-library dependency' >&2; exit 1
    fi
  fi
  if [[ "$target" == x86_64-unknown-linux-musl ]]; then
    "target/$target/release/cliary" --version
    CLIARY_TEST_BINARY="$PWD/target/$target/release/cliary" python scripts/test_install.py
  elif [[ "$target" == aarch64-unknown-linux-musl ]]; then
    qemu-aarch64 "target/$target/release/cliary" --version
    qemu-aarch64 "target/$target/release/cliary" search 'disk usage'
  fi
  python scripts/ci-package.py "$target"
  rm -rf "target/$target" "$ZIG_GLOBAL_CACHE_DIR"
  rustup target remove "$target"
done
python scripts/ci-package.py --finalize
