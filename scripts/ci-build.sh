#!/usr/bin/env bash
set -euo pipefail

export PATH="$CARGO_HOME/bin:$PWD/.ci-venv/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.94.1
export CARGO_ZIGBUILD_PYTHON_PATH="$PWD/.ci-venv/bin/python"
mkdir -p dist .ci-reports
report_resources() {
  python3 - <<'PY'
import json, os
from pathlib import Path
cgroup = next(line.split(':', 2)[2] for line in Path('/proc/self/cgroup').read_text().splitlines() if line.startswith('0:'))
directory = Path('/sys/fs/cgroup') / cgroup.lstrip('/')
def read_number(name):
    path = directory / name
    return int(path.read_text()) if path.exists() else None
disk = os.statvfs('.')
report = {'memory_peak_bytes': read_number('memory.peak'),
          'swap_peak_bytes': read_number('memory.swap.peak'),
          'memory_events': (directory / 'memory.events').read_text(),
          'cpu_stat': (directory / 'cpu.stat').read_text(),
          'work_disk_used_bytes': (disk.f_blocks - disk.f_bfree) * disk.f_frsize}
Path('.ci-reports/resources.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
PY
  df -h .
  du -sh target "$RUSTUP_HOME" "$CARGO_HOME" .ci-venv .ci-sdk 2>/dev/null || true
}
trap report_resources EXIT

python3 - <<'PY'
import os
from pathlib import Path
cgroup = next(line.split(':', 2)[2] for line in Path('/proc/self/cgroup').read_text().splitlines() if line.startswith('0:'))
directory = Path('/sys/fs/cgroup') / cgroup.lstrip('/')
for name, expected in [('memory.max', 500 * 1024**2), ('memory.swap.max', 1024**3)]:
    actual = (directory / name).read_text().strip()
    if actual != str(expected):
        raise SystemExit(f'Resource limit not enforced: {name}={actual}, expected {expected}')
quota, period = (directory / 'cpu.max').read_text().split()
if quota != period:
    raise SystemExit(f'CPU quota not enforced: {quota}/{period}')
disk = os.statvfs('.')
if disk.f_blocks * disk.f_frsize > 5 * 1024**3:
    raise SystemExit('Build filesystem exceeds the 5 GiB limit')
print('Verified kernel limits: 500 MiB RAM, 1 GiB swap, 1 CPU, at most 5 GiB build disk')
PY

python3 -m venv .ci-venv
python -m pip install --disable-pip-version-check \
  PyYAML==6.0.2 jsonschema==4.23.0 tomli==2.2.1 \
  cargo-zigbuild==0.23.4 ziglang==0.16.0

rustup_url=https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
curl --fail --location --retry 3 "$rustup_url" -o "$TMPDIR/rustup-init"
curl --fail --location --retry 3 "$rustup_url.sha256" -o "$TMPDIR/rustup-init.sha256"
(cd "$TMPDIR"; sha256sum --check rustup-init.sha256)
chmod +x "$TMPDIR/rustup-init"
"$TMPDIR/rustup-init" -y --no-modify-path --profile minimal --default-toolchain "$RUSTUP_TOOLCHAIN"
rm "$TMPDIR/rustup-init" "$TMPDIR/rustup-init.sha256"
rustc --version
cargo zigbuild --version
python -m ziglang version

python scripts/validate_catalog.py
python scripts/test_ci_package.py
cargo run --locked -p cliary-catalog --bin cliary-catalog-build -- validate
cargo test --workspace --locked
bash -n crates/cliary-cli/shell/bash.sh
zsh -n crates/cliary-cli/shell/zsh.sh
fish -n crates/cliary-cli/shell/fish.fish

if [[ "$CLIARY_PACKAGE" != true ]]; then exit 0; fi
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
