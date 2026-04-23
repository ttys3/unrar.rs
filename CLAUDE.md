# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`unrar-ng` — actively-maintained fork of the [`unrar`](https://crates.io/crates/unrar) crate. Published as `unrar-ng` but the `lib name` is still `unrar`, so downstream `use unrar::Archive;` keeps working. The headline feature over upstream is the batch extraction API (`OpenArchive::extract_all` / `extract_all_with_callback`), which matches native `unrar x` CLI throughput and is ~5.6x faster than the per-file API for archives with many small files.

Two crates (connected by a `path` dependency — there is no Cargo `[workspace]`):

- `unrar-ng` (root `Cargo.toml`) — high-level safe wrapper.
- `unrar-ng-sys` (`unrar_sys/`) — `#![no_std]` FFI crate (with a default `std` feature) that statically links the vendored UnRAR C++ source (`unrar_sys/vendor/unrar/`) compiled via the `cc` crate in `unrar_sys/build.rs`.

MSRV: `1.94` (both crates).

## Common commands

```bash
# Build everything (compiles the vendored C++ on the first pass)
cargo build

# Full test suite (integration tests live in tests/, require data/ fixtures)
cargo test

# Only the low-level FFI crate. The `--package` value is the Cargo package
# name (`unrar-ng-sys`), NOT the `lib` name `unrar_sys` which is only what
# you `use` from Rust — passing `--package=unrar_sys` errors out with
# "package ID specification did not match any packages".
cargo test --package=unrar-ng-sys

# Run a single integration test
cargo test --test packed_layout -- hash_type_is_a_valid_enum_value

# Examples
cargo run --example basic_list -- data/version.rar
# batch_extract refuses an existing dest dir — point it at a fresh path:
cargo run --example batch_extract -- archive.rar ./out-new

# Benchmark (criterion). Optionally point at a large archive:
UNRAR_BENCH_ARCHIVE=/path/to/kernel.rar cargo bench --bench extract_benchmark
```

CI matrix: Windows 2022/2025, macOS 14/15/15-intel/26, Ubuntu 24.04 (`.github/workflows/ci.yml`).

## Architecture

### High-level API — typestate pattern (`src/`)

`Archive<'a>` (`src/archive.rs`) does no I/O. Its `open_*` methods return an `OpenArchive<Mode, Cursor>` (`src/open_archive.rs`) that encodes the DLL's state machine at the type level:

- **`Mode` ∈ `{List, ListSplit, Process}`** — maps to `RAR_OM_LIST` / `RAR_OM_LIST_INCSPLIT` / `RAR_OM_EXTRACT`.
- **`Cursor` ∈ `{CursorBeforeHeader, CursorBeforeFile}`** — enforces the `RARReadHeaderEx` → `RARProcessFile` alternation.

`List` / `ListSplit` archives implement `Iterator`; `Process` archives don't (payload methods like `read`, `extract`, `extract_to` need ownership to advance the cursor).

Batch extraction bypasses the state machine entirely: `OpenArchive<Process, CursorBeforeHeader>::extract_all{,_with_callback}` calls the custom `RARExtractAll` / `RARExtractAllW` functions added to the vendored C++ (see patches below). The callback variant installs a Rust trampoline via `RARSetCallback` and dispatches `UCM_EXTRACTFILE{,_OK,_ERR}` messages as `ExtractEvent` values.

### Platform path split (`src/pathed/`)

RAR DLL path APIs come in narrow-char and wide-char flavors, and the right choice is platform-dependent:

- **Linux / NetBSD** (`pathed/linux.rs`): uses `CString` + `RARProcessFile` / `RARExtractAll` (8-bit). Wide-char extraction of Unicode filenames into a directory is broken on Linux upstream, so the Linux path constructs the *full* destination path instead of passing a base directory.
- **Everything else — macOS, Windows, other BSDs** (`pathed/all.rs`): uses `WideCString` + `RARProcessFileW` / `RARExtractAllW`.

`OpenArchiveDataEx::new` in `unrar_sys/src/lib.rs` has the mirror split (`*const c_char` on Linux/NetBSD, `*const wchar_t` elsewhere). When adding anything FFI path-related, the change has to land in both `pathed/linux.rs` and `pathed/all.rs`.

### FFI layout — `#[repr(C, packed(1))]` is mandatory

Every struct in `unrar_sys/src/lib.rs` that crosses the FFI boundary (`HeaderData`, `HeaderDataEx`, `OpenArchiveData`, `OpenArchiveDataEx`) must be `#[repr(C, packed(1))]` because `vendor/unrar/dll.hpp` declares them inside `#pragma pack(push, 1)`. Plain `#[repr(C)]` inserts 4 bytes of natural alignment padding before the first pointer field and silently shifts every subsequent field — tests only looked at pre-pointer fields for years, which is why the drift was latent.

Rules when touching these structs:

1. Keep `#[repr(C, packed(1))]`. The compile-time `offset_of!` assertions at the bottom of `unrar_sys/src/lib.rs` will refuse to compile if the layout drifts on 64-bit targets.
2. **Never** take a reference to a packed field. Copy it into a local with a value read first (`let flags = hdr.flags;`) or use `&raw const` for wide-char arrays you need a pointer to (see `HeaderDataEx::from` in `open_archive.rs` for the idiom).
3. When adding a new field, append it at the end and extend the `offset_of!`/`size_of!` assertion block.
4. `tests/packed_layout.rs` is the regression net — it reads fields that sit *after* the first pointer (`hash_type`, `redir_type`, `mtime_low`, etc.) and asserts documented ranges. Keep those assertions if you modify the struct.

### Vendored UnRAR source

`unrar_sys/vendor/unrar/` is a **pristine** RARLab source tree plus a small set of fork patches listed in `unrar_sys/vendor/patches.txt`. The patches (as full-length git SHAs) add:

1. Two small upstream-fix cherry-picks and one macOS Intel build fix.
2. The batch-extraction feature chain: `RARExtractAll`/`W` (dll.cpp/hpp/def), a perf pass over the extraction loop, and the `UCM_EXTRACTFILE{,_OK,_ERR}` callbacks.

To upgrade to a new UnRAR release, run `./unrar_sys/vendor/upgrade.sh <tarball-url>` from a clean working tree — it extracts the tarball over `unrar/` and cherry-picks every hash in `patches.txt` (order matters; 4 must come before 5 and 6). The vendor README has the full procedure.

`unrar_sys/build.rs` hard-codes the list of `.cpp` files to compile. New upstream versions may add files (e.g. `largepage.cpp` in 7.x, `motw.cpp` Windows-only) — the list in `build.rs` must be updated to match.

## Release / changelog

- Changelog is **auto-generated** by `git-cliff` using `cliff.toml` — do not edit `CHANGELOG.md` by hand. Commits must follow Conventional Commits or they're filtered out (`filter_unconventional = true`).
- `unrar-ng` and `unrar-ng-sys` are version-locked at the same `X.Y.Z`. A single bump has to update **four** places across two `Cargo.toml` files: root `[package].version`, root `[dependencies.unrar_sys].version`, root `[dev-dependencies].unrar_sys.version`, and `unrar_sys/Cargo.toml` `[package].version`. Use `/bump-version` — it handles all four.
- `unrar_sys` is also declared as a **dev-dependency** of the root crate (with the same `package = "unrar-ng-sys"` re-aliasing) purely so that `tests/packed_layout.rs` can `use unrar_sys::*` to hit the raw FFI — integration tests only see what the library re-exports, and the main crate deliberately does not re-export `unrar_sys`.
