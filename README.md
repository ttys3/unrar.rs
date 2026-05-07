# unrar-ng

[![crates.io](https://img.shields.io/crates/v/unrar-ng.svg)](https://crates.io/crates/unrar-ng)
[![API docs](https://docs.rs/unrar-ng/badge.svg)](https://docs.rs/unrar-ng)
[![build](https://github.com/ttys3/unrar.rs/workflows/ci/badge.svg)](https://github.com/ttys3/unrar.rs/actions?query=workflow%3Aci)
[![MIT license](https://img.shields.io/badge/license-MIT-blue.svg)](./README.md)

> **Actively maintained fork** of [`unrar`](https://crates.io/crates/unrar) / [muja/unrar.rs](https://github.com/muja/unrar.rs).
> This fork continues active development with performance improvements, extraction progress callbacks,
> and updates to the latest UnRAR source releases.
>
> Add to your `Cargo.toml`:
>
> ```toml
> [dependencies]
> unrar-ng = "0.7"
> ```
>
> Then `use unrar_ng::Archive;` in your code.
>
> ### Breaking change in 0.7
>
> The library targets were renamed: `unrar` → `unrar_ng`, `unrar_sys` → `unrar_ng_sys`. Code written against 0.6.x using `use unrar::Archive;` no longer compiles by default. Two migration paths:
>
> **1. Recommended (clean)** — update both Cargo.toml and source:
>
> ```toml
> [dependencies]
> unrar-ng = "0.7"
> ```
>
> ```rust
> use unrar_ng::Archive;
> ```
>
> **2. Minimal-change** — keep `use unrar::Archive;` source by aliasing the dep:
>
> ```toml
> [dependencies]
> unrar = { package = "unrar-ng", version = "0.7" }
> # Only if you also depend on the FFI crate directly:
> unrar_sys = { package = "unrar-ng-sys", version = "0.7" }
> ```
>
> Cargo's dep-rename mechanism makes the consumer-side `extern crate` / `use` name follow the dep key, regardless of the dependency's `[lib] name`, so existing `use unrar::Archive;` / `use unrar_sys::*;` lines continue to work.

High-level wrapper around the unrar C library provided by [rarlab](http://rarlab.com).

This library can only *extract* and *list* archives, it cannot *create* them.

## Why This Fork?

The primary motivation for this fork is **batch extraction performance**.

The original crate uses the UnRAR DLL's per-file API (`RARReadHeaderEx` + `RARProcessFile`), which internally calls `SearchBlock(HEAD_FILE)` on every iteration. This causes redundant traversal of archive block headers, resulting in severe performance degradation when extracting archives with many small files.

`unrar-ng` adds a new batch extraction API (`extract_all`) that uses the same efficient traversal loop as the native `unrar` CLI, completely eliminating this overhead.

### Performance: `unrar` crate vs `unrar-ng` (Linux kernel v7.0 source, ~94,000 files)

End-to-end extraction benchmark comparing the original [`unrar`](https://crates.io/crates/unrar) crate against `unrar-ng`. Test file: `kernel-v7.0.rar` (~94,000 files), created from the Linux kernel v7.0 source tree (downloaded from <https://github.com/torvalds/linux/archive/refs/tags/v7.0.zip>). The native `unrar` CLI and `unzip` are included as reference baselines.

| Backend | Executed (wall) | User CPU | Sys CPU |
|---------|-----------------|----------|---------|
| Native `unrar` CLI (reference) | **7.15 s** | 7.07 s | 1.25 s |
| Original `unrar` crate (per-file API) | 70.69 s | 68.03 s | 4.71 s |
| **`unrar-ng` (`extract_all`)** | **6.88 s** | 6.74 s | 1.35 s |

`unzip` on the same source tree (`unzip v7.0.zip`) takes **7.17 s** for reference.

Key takeaways:

- **~10.3x faster** than the original `unrar` crate (70.69 s → 6.88 s).
- On par with the native `unrar` CLI (6.88 s vs 7.15 s).
- Comparable to `unzip` extracting the same content (6.88 s vs 7.17 s).

For a detailed technical analysis, see [Batch Extraction Performance Optimization](./docs/2026-04-15_batch-extraction-performance-optimization.md).

<details>
<summary>Test environment</summary>

Tested under **tmpfs** to avoid filesystem I/O impact. Wall / user / sys timings come from the **fish shell** built-in `time` command.

**Software**

| Component | Version |
|-----------|---------|
| Host OS kernel | Linux 7.0.3 |
| Shell | fish |
| `rar` / `unrar` | 7.22 |
| `unzip` | UnZip 6.00 of 20 April 2009, by Info-ZIP |

**Hardware**

| Component | Spec |
|-----------|------|
| CPU | 12th Gen Intel(R) Core(TM) i7-12700 |
| RAM | 32 GB |

**Archive creation (for completeness)**

`rar a -r kernel-v7.0.rar ./linux-7.0/` — 20.12 s wall (102.85 s user, 10.30 s sys).

</details>

## Quick Example

```rust,no_run
use unrar_ng::Archive;

let archive = Archive::new("large_archive.rar")
    .open_for_processing()
    .expect("Failed to open archive");

archive.extract_all("./output")
    .expect("Failed to extract");
```

---

Please look inside the [examples directory](./examples) to see how to use this library.
Specifically the [**lister**](./examples/lister.rs) example is well documented and advanced!

Basic example to list archive entries:

```rust,no_run
use unrar_ng::Archive;

fn main() {
    for entry in Archive::new("archive.rar").open_for_listing().unwrap() {
        println!("{}", entry.unwrap());
    }
}
```

Run this example: `cargo run --example basic_list path/to/archive.rar`.
You can create an archive by using the `rar` CLI: `rar a archive.rar .`

# Overview

The primary type in this crate is [`Archive`]
which denotes an archive on the file system. `Archive` itself makes no
FS operations, unless one of the `open` methods are called, returning
an [`OpenArchive`].

# Archive

The [`Archive`] struct provides two major classes of methods:

   1. methods that do not touch the FS. These are opinionated utility methods
        that are based on RAR path conventions out in the wild. Most commonly, multipart
        files usually have extensions such as `.part08.rar` or `.r08.rar`. Since extracting
        must start at the first part, it may be helpful to figure that out using, for instance,
        [`archive.as_first_part()`](Archive::as_first_part)
   2. methods that open the underlying path in the specified mode
        (possible modes are [`List`], [`ListSplit`] and [`Process`]).
        These methods have the word `open` in them, are fallible operations,
        return [`OpenArchive`] inside a `Result` and are as follows:
        - [`open_for_listing`](Archive::open_for_listing) and
            [`open_for_listing_split`](Archive::open_for_listing_split): list the archive
            entries (skipping over content/payload)
        - [`open_for_processing`](Archive::open_for_processing): process archive entries
            as well as content/payload
        - [`break_open`](Archive::break_open): read archive even if an error is returned,
            if possible. The [`OpenMode`] must be provided
            explicitly.

# OpenArchive
An archive is opened in one of these three modes: [`List`], [`ListSplit`] or [`Process`].
This library does not provide random access into archives. Instead, files inside the archive
can only be processed as a stream, unidirectionally, front to back, alternating between
[`ReadHeader`] and [`ProcessFile`] operations (as dictated by the underlying C++ library).  

That is the idea behind cursors:

## OpenArchive: Cursors

Via cursors, the archive keeps track what operation is permitted next:
   - [`CursorBeforeHeader`] -> [`ReadHeader`]
   - [`CursorBeforeFile`] -> [`ProcessFile`]

The library enforces this by making
use of the [typestate pattern](https://cliffle.com/blog/rust-typestate/). An archive, once
opened, starts in the `CursorBeforeHeader` state and, thus, must have its [`read_header`] method
called, which returns a new `OpenArchive` instance in the `CursorBeforeFile` state that only
exposes methods that internally map to the `ProcessFile` operation.
Which methods are accessible in each step depends on the archive's current state and the
mode it was opened in.

## Available methods for Open mode/Cursor position combinations
Here is an overview of what methods are exposed for the OpenMode/Cursor combinations:

| Open mode↓ ╲ Cursor position→| before header   | before file                                                            |
|------------------------------|-----------------|------------------------------------------------------------------------|
| [`List`], [`ListSplit`]      | [`read_header`] | [`skip`]                                                               |
| [`Process`]                  | [`read_header`] | [`skip`], [`read`], [`extract`], [`extract_to`], [`extract_with_base`] |

## OpenArchive: Iterator

Archives opened in [`List`] or [`ListSplit`] mode also implement [`Iterator`] whereas archives in
[`Process`] mode do not (though this may change in future releases). That is because the first
two will read and return headers while being forced to skip over the payload whereas the latter
has more sophisticated processing possibilities that's not easy to convey using an [`Iterator`].

# Example

For more sophisticated examples, please look inside the `examples/` folder.

Here's what a function that returns the first content of a file could look like:

```rust
fn first_file_content<P: AsRef<Path>>(path: P) -> UnrarResult<Vec<u8>> {
    let archive = Archive::new(&path).open_for_processing()?; // cursor: before header
    let archive = archive.read_header()?.expect("empty archive"); // cursor: before file
    dbg!(&archive.entry().filename);
    let (data, _rest) = archive.read()?; // cursor: before header
    Ok(data)
}
# use std::path::Path;
# use unrar_ng::{Archive, UnrarResult};
#
# let data = first_file_content("data/version.rar").unwrap();
# assert_eq!(std::str::from_utf8(&data), Ok("unrar-0.4.0"));
```

[`read_header`]: OpenArchive::read_header
[`skip`]: OpenArchive::skip
[`read`]: OpenArchive::read
[`extract`]: OpenArchive::extract
[`extract_to`]: OpenArchive::extract_to
[`extract_with_base`]: OpenArchive::extract_with_base
[`ReadHeader`]: unrar_ng_sys::RARReadHeaderEx
[`ProcessFile`]: unrar_ng_sys::RARProcessFileW

# Features

- [x] Multipart files
- [x] Listing archives
- [x] Extracting them
- [x] Reading them into memory (without extracting)
- [x] Testing them
- [x] Encrypted archives with password
- [x] Linked statically against the unrar source.
- [x] Build unrar C++ code from source
- [x] Basic functionality that operates on filenames / paths (without reading archives)
- [x] Documentation / RustDoc
- [x] Test Suite
- [x] utilizes type system to enforce correct usage
- [ ] Well-designed errors (planned)
- [ ] TBD

# Non-Features
As this library is only a wrapper, these following features
are not easily feasible and as such not planned:

- Creating archives
- Random access into arbitrary archive entries
- Pure Rust implementation
- Processing archives from a file descriptor / fs::File handle
- Processing archives from a byte stream

# Contributing

Feel free to contribute! If you detect a bug, open an issue.

Pull requests are also welcome!

# Help

If you need help using the library, feel free to create a new discussion or open an issue.

# License

The parts authored by this library's contributors are licensed under either of

  * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
    http://www.apache.org/licenses/LICENSE-2.0)
  * MIT license ([LICENSE-MIT](LICENSE-MIT) or
    http://opensource.org/licenses/MIT)

at your option.

The embedded [C/C++ library](./unrar_sys/vendor/unrar) uses its own license. For more informations, see its [license file](./unrar_sys/vendor/unrar/license.txt).

# Acknowledgements

`unrar-ng` is a fork of the original [`unrar`](https://crates.io/crates/unrar) crate at [muja/unrar.rs](https://github.com/muja/unrar.rs). Huge thanks to [@muja](https://github.com/muja) and the original contributors — `unrar-ng` builds directly on their work.
