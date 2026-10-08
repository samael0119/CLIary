#!/bin/sh
set -eu

# Release packaging substitutes this marker. For a source checkout, set CLIARY_REPO=owner/repo.
repo=${CLIARY_REPO:-@GITHUB_REPOSITORY@}
install_dir=${CLIARY_INSTALL_DIR:-"$HOME/.local/bin"}
history=ask
scan=ask
import_history=ask
for arg in "$@"; do
  case "$arg" in
    --scan) scan=yes ;;
    --no-scan) scan=no ;;
    --enable-history) history=yes ;;
    --no-history) history=no; import_history=no ;;
    --no-import-history) import_history=no ;;
    --help) printf '%s\n' 'Usage: install.sh [--scan|--no-scan] [--enable-history|--no-history] [--no-import-history]' 'History import is optional: preview first, then confirm. --no-history skips capture and the import guide.'; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$arg" >&2; exit 2 ;;
  esac
done
case "$repo" in
  @GITHUB_REPOSITORY@|'') printf '%s\n' 'Set CLIARY_REPO=owner/repo when running from a source checkout.' >&2; exit 2 ;;
esac

case "$(uname -s):$(uname -m)" in
  Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin:x86_64) target=x86_64-apple-darwin ;;
  Darwin:arm64) target=aarch64-apple-darwin ;;
  *) printf '%s\n' 'Unsupported operating system or architecture.' >&2; exit 2 ;;
esac

archive="cliary-$target.tar.gz"
base="https://github.com/$repo/releases/latest/download"
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM
curl --fail --location --silent --show-error "$base/$archive" -o "$temp_dir/$archive"
curl --fail --location --silent --show-error "$base/SHA256SUMS" -o "$temp_dir/SHA256SUMS"
expected=$(awk -v file="$archive" '$2 == file {print $1}' "$temp_dir/SHA256SUMS")
[ -n "$expected" ] || { printf '%s\n' 'Release checksum is missing.' >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$temp_dir/$archive" | awk '{print $1}')
else
  actual=$(shasum -a 256 "$temp_dir/$archive" | awk '{print $1}')
fi
[ "$actual" = "$expected" ] || { printf '%s\n' 'Archive checksum mismatch.' >&2; exit 1; }
tar -xzf "$temp_dir/$archive" -C "$temp_dir" cliary
mkdir -p "$install_dir"
install -m 755 "$temp_dir/cliary" "$install_dir/cliary"
printf 'Installed CLIary at %s/cliary\n' "$install_dir"

if [ "$scan" = ask ]; then
  if [ -t 0 ]; then
    printf 'Scan installed CLI tools now? [Y/n] '
    answer=''
    IFS= read -r answer || true
    case "$answer" in [nN]|[nN][oO]) scan=no ;; *) scan=yes ;; esac
  else
    scan=no
    printf '%s\n' 'Non-interactive install: initial scan skipped. Run cliary scan when ready.'
  fi
fi
if [ "$scan" = yes ]; then
  printf '%s\n' 'Scanning installed CLI tools...'
  "$install_dir/cliary" scan || printf '%s\n' 'Initial scan failed; run cliary scan later.' >&2
else
  "$install_dir/cliary" internal skip-scan
fi

if [ "$history" = ask ]; then
  if [ -t 0 ]; then
    printf 'Enable tool-level shell history for this user? [Y/n] '
    answer=''
    IFS= read -r answer || true
    case "$answer" in [nN]|[nN][oO]) history=no ;; *) history=yes ;; esac
  else
    history=no
    printf '%s\n' 'Non-interactive install: shell history remains disabled. Run cliary setup shell --enable to opt in.'
  fi
fi
if [ "$history" = yes ]; then
  "$install_dir/cliary" setup shell --enable
fi
if [ "$import_history" != no ]; then
  if [ -t 0 ] && [ -t 2 ]; then
    "$install_dir/cliary" setup history || printf '%s\n' 'History guide did not finish; run cliary setup history later.' >&2
  else
    printf '%s\n' 'Existing Bash/Zsh/Fish history is not imported automatically.' 'Run cliary setup history in a terminal to select a file, preview counts and choose whether to import.' 'You can also preview and import from the Web History page; dated records enrich annual reports.'
  fi
else
  "$install_dir/cliary" internal skip-history-guide
fi
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'Add %s to PATH to run cliary from a new shell.\n' "$install_dir" ;;
esac
