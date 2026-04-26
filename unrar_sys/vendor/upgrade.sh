#!/usr/bin/env bash
#
# Upgrade vendored UnRAR source to a new upstream tarball and re-apply
# all fork patches under unrar_sys/vendor/patches/.
#
# Usage: ./upgrade.sh <tarball url>
#   e.g.  ./upgrade.sh https://www.rarlab.com/rar/unrarsrc-7.2.5.tar.gz
#
# This replaces the legacy "patches.txt + git cherry-pick" flow.
# The patch series is now stored as numbered .patch files under
# unrar_sys/vendor/patches/, applied in name order via `git apply --index`.

set -euo pipefail

usage() { echo "Usage: $0 <tarball url>"; exit "$1"; }
fatal() { echo >&2 "fatal: $1, aborting..."; exit 1; }

# Argument checks come first so `--help` and a missing-URL prompt
# are not gated behind the dirty-tree / repo-root checks below.
[ "${1:-}" = "" ] && usage 1
[ "$1" = "--help" ] && usage 0

# Anchor to the repo root — git apply --directory and all path
# arguments below are interpreted relative to it.
cd "$(git rev-parse --show-toplevel)" || fatal "not in a git repository"

# Dirty-tree check: split into two steps so that a failing
# `git status` (corrupted .git, permissions) is not silently
# swallowed by the trailing `|| true`.
porcelain=$(git status --porcelain) || fatal "git status failed"
dirty=$(printf '%s\n' "$porcelain" | grep -v '^??' || true)
[ -n "$dirty" ] && fatal "git repository must not be dirty (staged/modified detected)"

# Stricter check inside vendor/unrar/: the script will `rm -rf` this
# directory, which deletes both tracked and untracked files. Refuse to
# run if the maintainer has any local scratch (e.g. a debug .cpp not
# yet committed) under vendor/unrar/, regardless of tracked status.
unrar_local=$(git status --porcelain unrar_sys/vendor/unrar/ || true)
[ -n "$unrar_local" ] && fatal "unrar_sys/vendor/unrar/ has local changes (tracked or untracked); these would be destroyed by rm -rf"

vendor_dir="unrar_sys/vendor"
unrar_dir="$vendor_dir/unrar"
patches_dir="$vendor_dir/patches"

# === Pre-flight fail-fast checks (run BEFORE touching vendor/) ===
# patches/ must exist and be non-empty, otherwise we'd fatal only
# after the vendor tree is half-replaced, leaving a broken state.
[ -d "$patches_dir" ] && [ -n "$(ls -A "$patches_dir" 2>/dev/null)" ] \
    || fatal "patches dir empty or missing: $patches_dir"

shopt -s nullglob
patches=("$patches_dir"/*.patch)
shopt -u nullglob
[ "${#patches[@]}" -gt 0 ] || fatal "no .patch files under $patches_dir"

# === Extract tarball into a private staging dir ===
# Direct `tar -C "$vendor_dir"` is unsafe: tarballs with a
# top-level flat layout (or macOS __MACOSX/ metadata) would
# pollute vendor/patches/, vendor/upgrade.sh etc.
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT
curl -fL "$1" -o "$staging/src.tar.gz" || fatal "download failed"
tar xf "$staging/src.tar.gz" -C "$staging" || fatal "extract failed"

# Locate the single top-level directory (typically `unrarsrc-X.Y.Z/`).
# Use `-print -quit` instead of `| head -1` so SIGPIPE under
# `set -o pipefail` does not falsely fail the pipeline.
inner_dir=$(find "$staging" -mindepth 1 -maxdepth 1 -type d ! -name '__*' -print -quit)
[ -d "$inner_dir" ] || fatal "tarball layout unexpected: no single top-level dir"

# === Replace vendor/unrar with the pristine extract ===
# `rm -rf` only touches the working tree. The git index still
# holds entries for every .cpp/.hpp from the previous version —
# if upstream has dropped a file (e.g. 7.x dropped/renamed
# motw.cpp / largepage.cpp variants on different platforms),
# a plain `git add` won't notice it and the next commit would
# silently retain ghost files. Untrack first, then `git add -A`.
git rm -rf --cached --ignore-unmatch "$unrar_dir" >/dev/null 2>&1 || true
rm -rf "$unrar_dir"
mv "$inner_dir" "$unrar_dir"

# `-A` stages additions, modifications, and deletions — the
# index will be byte-for-byte the new pristine tree.
git add -A "$unrar_dir"

# === Apply each patch in numeric order ===
# `--check` first for a clearer error message (e.g. "file removed
# upstream") before mutating the index/tree.
# `--whitespace=nowarn` pins the policy regardless of the user's
# global `apply.whitespace` setting (some hardened gitconfigs default
# to `error`, which would reject our format-patch output's harmless
# trailing whitespace inherited from the upstream commits).
for patch in "${patches[@]}"; do
    echo "checking  $patch"
    git apply --index --check --whitespace=nowarn --directory="$unrar_dir" "$patch" \
        || fatal "patch $patch does not apply cleanly (file removed or content drifted upstream?)"
    echo "applying  $patch"
    git apply --index --whitespace=nowarn --directory="$unrar_dir" "$patch" \
        || fatal "failed to apply $patch"
done

echo
echo "ok: vendor upgraded and ${#patches[@]} patches applied"
