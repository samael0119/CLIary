#!/usr/bin/env bash
# Bound build processes with cgroup v2 and an isolated 5 GiB filesystem.
set -euo pipefail

: "${RUNNER_TEMP:?RUNNER_TEMP is required}"
: "${GITHUB_RUN_ID:?GITHUB_RUN_ID is required}"
: "${GITHUB_RUN_ATTEMPT:?GITHUB_RUN_ATTEMPT is required}"
[[ "$GITHUB_RUN_ID" =~ ^[0-9]+$ && "$GITHUB_RUN_ATTEMPT" =~ ^[0-9]+$ ]]
ci_root="${RUNNER_TEMP%/}/cliary-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT"
ci_unit="cliary-build-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT"
privileged=()
if [[ $(id -u) != 0 ]]; then privileged=(sudo -n); fi

case "${1:-}" in
  prepare)
    printf 'CLIARY_CI_ROOT=%s\n' "$ci_root" >> "$GITHUB_ENV"
    echo 'Runner resources before build:'
    free -m
    df -h "$RUNNER_TEMP"
    test -f /sys/fs/cgroup/cgroup.controllers || {
      echo 'cgroup v2 is required for the memory and swap limits.' >&2; exit 1;
    }
    grep -qw memory /sys/fs/cgroup/cgroup.controllers
    "${privileged[@]}" true
    missing=()
    for command in cc python3 curl readelf mkfs.ext4 mountpoint xz zsh fish qemu-aarch64; do
      if ! command -v "$command" >/dev/null; then missing+=("$command"); fi
    done
    if ! python3 -c 'import venv, ensurepip' 2>/dev/null; then missing+=(python3-venv); fi
    if ((${#missing[@]})); then
      echo "Installing missing host prerequisites: ${missing[*]}"
      command -v apt-get >/dev/null || {
        echo 'Install build-essential, python3-venv, curl, binutils, e2fsprogs, zsh, fish and qemu-user first.' >&2; exit 1;
      }
      "${privileged[@]}" env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=l apt-get update -qq
      "${privileged[@]}" env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=l apt-get install -y --no-install-recommends --no-upgrade \
        build-essential python3-venv curl binutils e2fsprogs xz-utils zsh fish qemu-user
    fi
    # Check venv support before allocating the build disk.
    python3 -c 'import venv, ensurepip'
    available=$(df -PB1 "$RUNNER_TEMP" | awk 'NR==2 {print $4}')
    if ((available < 5368709120)); then
      echo 'At least 5 GiB free disk space is required; the existing system and runner are outside this budget.' >&2
      exit 1
    fi
    mkdir -p "$ci_root/work"
    truncate -s 5G "$ci_root/work.img"
    mkfs.ext4 -q -F -m 0 "$ci_root/work.img"
    "${privileged[@]}" mount -o loop,nosuid,nodev "$ci_root/work.img" "$ci_root/work"
    "${privileged[@]}" chown "$(id -u):$(id -g)" "$ci_root/work"
    mkdir -p "$ci_root/work/source"
    git archive HEAD | tar -x -C "$ci_root/work/source"
    ;;
  run)
    mkdir -p "$ci_root/work/tmp" "$ci_root/work/cache"
    trap '"${privileged[@]}" systemctl stop "$ci_unit" 2>/dev/null || true' EXIT INT TERM
    # Toolchains, Python packages, SDK, temporary files and build output all use the bounded disk.
    "${privileged[@]}" systemd-run --quiet --wait --pipe \
      --unit="$ci_unit" --service-type=exec \
      --property="User=$(id -u)" --property="Group=$(id -g)" \
      --property="WorkingDirectory=$ci_root/work/source" \
      --property=MemoryAccounting=yes --property=MemoryMax=500M \
      --property=MemorySwapMax=1G --property=CPUQuota=100% \
      --property=OOMPolicy=stop --property=RuntimeMaxSec=9000 \
      --setenv="PATH=/usr/local/bin:/usr/bin:/bin" \
      --setenv="RUSTUP_HOME=$ci_root/work/rustup" \
      --setenv="CARGO_HOME=$ci_root/work/cargo" \
      --setenv="TMPDIR=$ci_root/work/tmp" \
      --setenv="XDG_CACHE_HOME=$ci_root/work/cache" \
      --setenv="ZIG_GLOBAL_CACHE_DIR=$ci_root/work/cache/zig" \
      --setenv="CLIARY_CONFIG_DIR=$ci_root/work/config" \
      --setenv="CLIARY_DATA_DIR=$ci_root/work/data" \
      --setenv="CLIARY_CACHE_DIR=$ci_root/work/cache/cliary" \
      --setenv="CARGO_BUILD_JOBS=1" --setenv="CARGO_INCREMENTAL=0" \
      --setenv="CARGO_PROFILE_DEV_DEBUG=0" --setenv="CARGO_PROFILE_TEST_DEBUG=0" \
      --setenv="CARGO_PROFILE_RELEASE_STRIP=symbols" \
      --setenv="PIP_NO_CACHE_DIR=1" \
      --setenv="CLIARY_BUILD_MACOS=${CLIARY_BUILD_MACOS:-true}" \
      --setenv="CLIARY_PACKAGE=${CLIARY_PACKAGE:-true}" \
      --setenv="GITHUB_SHA=$GITHUB_SHA" \
      /usr/bin/bash scripts/ci-build.sh
    ;;
  report)
    "${privileged[@]}" systemctl show "$ci_unit" \
      --property=Result --property=MemoryMax --property=MemorySwapMax \
      --property=MemoryPeak --property=MemorySwapPeak --property=CPUUsageNSec || true
    if mountpoint -q "$ci_root/work"; then
      df -h "$ci_root/work"
      du -sh "$ci_root/work"/* 2>/dev/null || true
    fi
    ;;
  cleanup)
    "${privileged[@]}" systemctl stop "$ci_unit" 2>/dev/null || true
    "${privileged[@]}" systemctl reset-failed "$ci_unit" 2>/dev/null || true
    if mountpoint -q "$ci_root/work"; then
      "${privileged[@]}" umount "$ci_root/work"
    fi
    # ci_root is derived exclusively from RUNNER_TEMP and validated numeric run identifiers.
    rm -rf -- "$ci_root"
    ;;
  *) echo 'Usage: ci-run-limited.sh prepare|run|report|cleanup' >&2; exit 2 ;;
esac
