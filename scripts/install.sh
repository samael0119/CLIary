#!/bin/sh
set -eu

# Release packaging substitutes this marker. For a source checkout, set CLIARY_REPO=owner/repo.
repo=${CLIARY_REPO:-@GITHUB_REPOSITORY@}
install_dir=${CLIARY_INSTALL_DIR:-"$HOME/.local/bin"}
history=ask
scan=ask
import_history=ask
from_dir=''
github=no
tag=${CLIARY_RELEASE_TAG:-}
while [ "$#" -gt 0 ]; do
  case "$1" in
    --scan) scan=yes ;;
    --no-scan) scan=no ;;
    --enable-history) history=yes ;;
    --no-history) history=no; import_history=no ;;
    --no-import-history) import_history=no ;;
    --github) github=yes ;;
    --from|--tag)
      option=$1
      [ "$#" -ge 2 ] && [ -n "$2" ] || { printf 'Missing value for %s\n' "$option" >&2; exit 2; }
      case "$2" in --*) printf 'Missing value for %s\n' "$option" >&2; exit 2 ;; esac
      case "$option" in --from) from_dir=$2 ;; --tag) tag=$2 ;; esac
      shift ;;
    --help)
      printf '%s\n' 'Usage: install.sh [--from DIRECTORY | --github] [--tag TAG] [--scan|--no-scan] [--enable-history|--no-history] [--no-import-history]' 'Private repository: set CLIARY_REPO=owner/repo and use --github after gh auth login.' 'Offline: --from DIRECTORY reads a platform archive and SHA256SUMS; no repository or login needed.' 'History import is optional: preview first, then confirm. --no-history skips capture and the import guide.'
      exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; exit 2 ;;
  esac
  shift
done
if [ -n "$from_dir" ]; then
  [ "$github" = no ] && [ -z "$tag" ] || { printf '%s\n' '--from cannot be combined with --github or --tag.' >&2; exit 2; }
else
  printf '%s\n' "$repo" | awk -F/ 'NF == 2 && $1 ~ /^[A-Za-z0-9_.-]+$/ && $2 ~ /^[A-Za-z0-9_.-]+$/ && $1 !~ /^-/ {ok=1} END {exit !ok}' || {
    printf '%s\n' 'Set CLIARY_REPO=owner/repo when running from a source checkout.' >&2; exit 2;
  }
fi
case "$tag" in -*) printf '%s\n' 'Release tag cannot start with a dash.' >&2; exit 2 ;; esac

legacy_target=''
case "$(uname -s):$(uname -m)" in
  Linux:x86_64) target=x86_64-unknown-linux-musl; legacy_target=x86_64-unknown-linux-gnu ;;
  Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-musl; legacy_target=aarch64-unknown-linux-gnu ;;
  Darwin:x86_64) target=x86_64-apple-darwin ;;
  Darwin:arm64) target=aarch64-apple-darwin ;;
  *) printf '%s\n' 'Unsupported operating system or architecture.' >&2; exit 2 ;;
esac

archive="cliary-$target.tar.gz"
temp_dir=$(mktemp -d)
install_temp=''
trap 'rm -rf "$temp_dir"; [ -z "$install_temp" ] || rm -f "$install_temp"' EXIT
trap 'exit 130' INT
trap 'exit 143' HUP TERM
if [ -n "$from_dir" ]; then
  cp "$from_dir/SHA256SUMS" "$temp_dir/SHA256SUMS"
elif [ "$github" = yes ]; then
  command -v gh >/dev/null 2>&1 || { printf '%s\n' 'Private downloads require GitHub CLI (gh). Install it and run gh auth login, or use --from DIRECTORY.' >&2; exit 1; }
  # Pin both assets to one release. Credentials and authenticated redirects are handled by gh.
  if [ -z "$tag" ]; then
    tag=$(gh release view --repo "$repo" --json tagName --jq .tagName) || {
      printf '%s\n' 'Cannot access the release. Check gh login, repository access, and published assets.' >&2; exit 1;
    }
  fi
  case "$tag" in ''|-*) printf '%s\n' 'Invalid release tag.' >&2; exit 1 ;; esac
  gh release download "$tag" --repo "$repo" --pattern SHA256SUMS --dir "$temp_dir"
else
  if [ -n "$tag" ]; then
    base="https://github.com/$repo/releases/download/$tag"
  else
    base="https://github.com/$repo/releases/latest/download"
  fi
  curl --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 120 --fail --location --silent --show-error "$base/SHA256SUMS" -o "$temp_dir/SHA256SUMS"
fi
# Prefer static Linux assets; still accept GNU packages from older releases.
if [ -n "$legacy_target" ] && ! awk -v file="$archive" '$2 == file {found=1} END {exit !found}' "$temp_dir/SHA256SUMS"; then
  archive="cliary-$legacy_target.tar.gz"
fi
expected=$(awk -v file="$archive" '$2 == file {print $1}' "$temp_dir/SHA256SUMS")
printf '%s\n' "$expected" | awk 'length($0) == 64 && $0 !~ /[^0-9a-f]/ {ok++} END {exit ok != 1}' || { printf '%s\n' 'Release checksum is missing.' >&2; exit 1; }
if [ -n "$from_dir" ]; then
  cp "$from_dir/$archive" "$temp_dir/$archive"
elif [ "$github" = yes ]; then
  gh release download "$tag" --repo "$repo" --pattern "$archive" --dir "$temp_dir"
else
  curl --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 120 --fail --location --silent --show-error "$base/$archive" -o "$temp_dir/$archive"
fi
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$temp_dir/$archive" | awk '{print $1}')
else
  actual=$(shasum -a 256 "$temp_dir/$archive" | awk '{print $1}')
fi
[ "$actual" = "$expected" ] || { printf '%s\n' 'Archive checksum mismatch.' >&2; exit 1; }
tar -xzf "$temp_dir/$archive" -C "$temp_dir" cliary
mkdir -p "$install_dir"
[ -f "$temp_dir/cliary" ] && [ ! -L "$temp_dir/cliary" ] || { printf '%s\n' 'Archive must contain a regular cliary binary.' >&2; exit 1; }
# Atomic replacement also works while an older Web server is running.
install_temp=$(mktemp "$install_dir/.cliary.XXXXXX")
install -m 755 "$temp_dir/cliary" "$install_temp"
mv -f "$install_temp" "$install_dir/cliary"
install_temp=''
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
