# Updating the vendored UnRAR source

This directory contains a pristine UnRAR source tree under [`unrar/`](./unrar/)
plus a series of fork-only patches under [`patches/`](./patches/) that are
re-applied on every upgrade.

## Upgrading to a new upstream release

```bash
./upgrade.sh https://www.rarlab.com/rar/unrarsrc-7.2.5.tar.gz
```

`upgrade.sh` does the following from the repo root:

1. Verify the working tree is clean.
2. Verify [`patches/`](./patches/) is non-empty (fail-fast before touching `unrar/`).
3. Download and extract the tarball into a private staging dir.
4. Untrack the old `unrar/` from the index (`git rm --cached --ignore-unmatch`)
   so files removed upstream do not linger as ghosts.
5. Replace `unrar/` with the pristine extract and `git add -A` it.
   The combined `git rm --cached` + clean `unrar/` rebuild also reaps any
   obsolete files left from previous flat-tar overlays (e.g. `array.hpp`,
   dropped upstream around the 6.2.8 → 7.x transition).
6. Apply each patch in `patches/000N-*.patch` in name order via
   `git apply --index --check` then `git apply --index`. `git apply` may
   print `trailing whitespace` warnings on stderr — these are advisory only,
   inherited from the original commits' diffs; the patch still applies and
   the script continues.

## Patch series convention

`patches/` holds numbered, format-patch–style files:

```
0001-fix-rar-open-archive-ex-bad-data-handle.patch
0002-fix-readheader-thread-safe-error.patch
0003-chore-guard-builtin-cpu-supports.patch
0004-feat-rar-extract-all-batch.patch
0005-perf-rar-extract-all-w-loop.patch
0006-feat-ucm-extractfile-callbacks.patch
```

The numeric prefix determines `git apply` order. Each file is a
self-contained `git format-patch` output (`From <hash>` / `From: <author>` /
`Subject: ...` headers + diff body), generated with
`--relative=unrar_sys/vendor/unrar` so paths inside the patch are
`a/dll.cpp` etc. and apply cleanly under
`git apply --directory=unrar_sys/vendor/unrar`.

### Note on patch 0004

`0004-feat-rar-extract-all-batch.patch` was generated from the original
commit `56263690` with a pathspec filter excluding the now-defunct
`unrar_sys/vendor/patches.txt`. (`patches.txt` was the legacy plaintext
list of cherry-pick hashes used by the previous upgrade flow — see the
git history before 0.7.x; the same data now lives implicitly in this
`patches/` directory.) The `From <hash>` header still references the
original commit, but the patch body is a strict subset of
`git show 56263690` — this is intentional, not a corruption.

## Editing or adding a patch

The patch series is the source of truth. Don't hand-edit `.patch` files —
hunk headers (`@@ -a,b +c,d @@`) are easy to miscount and `git apply`
won't recover. Regenerate instead:

1. `git worktree add --detach <tmpdir>` and `cd` into it.
2. Reset `vendor/unrar/` to the pristine upstream extract (or apply
   patches `0001..NN-1` first if your patch builds on earlier ones).
3. Make the change in `vendor/unrar/`.
4. `git commit` it on the temporary worktree.
5. `git format-patch -1 --output=unrar_sys/vendor/patches/000N-<slug>.patch \
       --relative=unrar_sys/vendor/unrar -- unrar_sys/vendor/unrar/`.
   The `From <40-char-hash>` header in the regenerated file is the
   *temporary* worktree commit's hash, not the original upstream/fork
   commit — that's expected for any patch except `0004-…`, whose `From`
   header is preserved by convention to point at `56263690` (see the
   "Note on patch 0004" section above).
6. Copy the regenerated `.patch` file back to the main worktree.
7. `git worktree remove --force <tmpdir>` to clean up.

If the patched files already exist in the main worktree (the typical case
for an in-flight regeneration where the change is already staged or
committed there), you can skip the manual edit step and instead `cp` the
files from the main worktree on top of the temporary worktree's
patched-up tree before calling `git format-patch`. This produces the same
final patch with less risk of inconsistent re-typing — see
`.claude/scripts/regen-0006.sh` (untracked, maintainer-only) for an
end-to-end example.

Note: `vendor/patches/*.patch` files are not in cargo's rebuild graph
(`build.rs` watches `vendor/unrar/` only). Editing a `.patch` file alone
does **not** trigger a `cargo build` recompile — you must re-run
`upgrade.sh` (or otherwise materialize the patch into `vendor/unrar/`)
before the change is observable in compiled artifacts.

After regenerating, dry-run `upgrade.sh` against the same upstream tarball
in another disposable worktree to confirm the full series still applies
cleanly end-to-end.
