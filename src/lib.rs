#![doc = include_str!("../README.md")]
//!
//! # Filename encoding on extraction
//!
//! ## Default behavior (`linux-batch-extract-utf8` feature ON, `linux-batch-extract-setlocale` OFF)
//!
//! On **Linux / FreeBSD / NetBSD / OpenBSD**, extracted filenames are written
//! to disk as **UTF-8 bytes, unconditionally** — regardless of the process's
//! `LANG` / `LC_CTYPE` / `LC_ALL`, regardless of the source archive's
//! filename encoding (RAR5 / RAR4-with-Unicode-field / RAR4-ANSI-only), and
//! regardless of whether the caller has invoked `setlocale` before. This is
//! implemented at the C level via vendor patch 0007: `WideToChar` and
//! `CharToWide` in `unicode.cpp` are routed through `WideToUtf` /
//! `UtfToWide` (locale-independent UTF-8 transforms — the same code path
//! macOS has used for years via the auto-defined `_APPLE` macro). The
//! crate **never calls `setlocale`** in this mode; the host process's
//! libc state is **never** modified.
//!
//! On **macOS / iOS**, the same `_APPLE` branch fires regardless of feature
//! flags — `WideToUtf` / `UtfToWide`, locale-independent.
//!
//! On **Windows**, extraction goes through `WideCharToMultiByte(CP_ACP, ...)`
//! and `CreateFile(LPCWSTR)` (wide-native NTFS), which honor the OS system
//! codepage (CP936 zh-CN, CP950 zh-TW, CP932 ja-JP) without any libc
//! `setlocale` involvement.
//!
//! ## Why UTF-8 by default on Linux/BSD?
//!
//! Modern Linux filesystem convention is UTF-8. Every userspace tool, every
//! desktop environment, every FS driver default, every `systemd`-shipped
//! distro since ~2010 expects UTF-8 byte names. Stock libunrar uses
//! `wcsrtombs`, which under the default `C` / `POSIX` locale emits literal
//! `_` for every non-ASCII wchar — mangling CJK / accented filenames into
//! runs of underscores on disk. Standalone `unrar` CLI dodges this with
//! `setlocale(LC_ALL, "")` in `main()` (`rar.cpp:8`), but a library cannot
//! do the same without mutating process-global libc state, perturbing
//! every other libc-locale-sensitive call (`strtod`, `strftime`,
//! `isalpha`, etc.) in the host process.
//!
//! Bypassing the locale-aware code path entirely (this crate's default)
//! avoids both the `_`-mangling bug and the libc-state pollution.
//!
//! Note that empirically the upstream bug only affects the **batch extract
//! paths** (`OpenArchive::extract_all` / `extract_all_with_callback`).
//! Per-file extract methods (`extract`, `extract_to`, `extract_with_base`)
//! are already locale-immune via a private-use-area round-trip in
//! `pathed/linux.rs` — the Rust wrapper builds the full destination path
//! before handing it to libunrar, and the byte-level round-trip preserves
//! arbitrary UTF-8 bytes without ever round-tripping through a real
//! locale-aware conversion. Listing, reading entries to memory, testing,
//! and skipping likewise don't exercise the buggy path.
//!
//! ## Opt-out: legacy non-UTF-8 LANG support
//!
//! Callers who need on-disk filename bytes encoded in a legacy codepage
//! (`zh_CN.GBK`, `ja_JP.eucJP`, etc.) — for example to feed a downstream
//! tool that expects locale-encoded byte names, or to mirror standalone
//! `unrar` CLI behavior under non-UTF-8 `LANG` settings — can opt out via
//! cargo features. Three useful configurations:
//!
//! ### 1. Default (recommended for ≥ 99% of callers)
//!
//! `linux-batch-extract-utf8` ON, `linux-batch-extract-setlocale` OFF.
//! Linux always writes UTF-8 bytes to disk, no `setlocale` ever called.
//!
//! ```toml
//! [dependencies]
//! unrar-ng = "0.7"
//! ```
//!
//! ### 2. CLI-equivalent locale-respecting behavior
//!
//! `linux-batch-extract-utf8` OFF, `linux-batch-extract-setlocale` ON.
//! libunrar's upstream `wcsrtombs` / `mbsrtowcs` path is restored, and the
//! crate calls `setlocale(LC_CTYPE, "")` once via [`std::sync::OnceLock`]
//! at the **first invocation of `extract_all` / `extract_all_with_callback`**,
//! with a fallback ladder (`C.UTF-8` / `C.utf8` / `en_US.UTF-8` /
//! `en_US.utf8`) for containers without `LANG` set. On-disk bytes follow
//! the host's `LANG`:
//! `zh_CN.GBK` → GBK byte names, `ja_JP.eucJP` → EUC-JP byte names,
//! `*.UTF-8` → UTF-8 byte names. Listing, per-file extract, reading to
//! memory, testing, and skipping do NOT trigger the locale init —
//! they're either locale-immune or don't write to disk.
//!
//! Note: this mode mutates process-global libc state (`LC_CTYPE`).
//! `setlocale` is not thread-safe against concurrent calls from other
//! threads; enabling this feature implicitly accepts that responsibility.
//!
//! ```toml
//! [dependencies]
//! unrar-ng = { version = "0.7", default-features = false, features = ["linux-batch-extract-setlocale"] }
//! ```
//!
//! ### 3. Stock upstream (caller-managed locale)
//!
//! Both features OFF. Bare upstream libunrar behavior. The caller is fully
//! responsible for invoking `setlocale(LC_CTYPE, "")` (or equivalent)
//! before calling this crate. If the caller does nothing, non-ASCII
//! filenames will be mangled to `_` under default `C` / `POSIX` locale
//! on the batch-extract paths.
//!
//! ```toml
//! [dependencies]
//! unrar-ng = { version = "0.7", default-features = false }
//! ```
//!
//! Both features ON together is permitted — the `setlocale` call becomes
//! effectively dead code (the patched `WideToUtf` path doesn't consult
//! locale anyway) but causes no harm.
//!
//! macOS / iOS / Windows are unaffected by either feature: their respective
//! `_APPLE` and `_WIN_ALL` branches in `unicode.cpp` run regardless,
//! neither goes through `wcsrtombs` or libc `setlocale`. The `libc`
//! optional dependency is target-gated to `cfg(all(unix, not(target_vendor
//! = "apple")))` and is not pulled into Apple / Windows builds.
//!
//! ## Cross-platform behavior summary
//!
//! | Target OS               | On-disk filename bytes                                  |
//! |-------------------------|----------------------------------------------------------|
//! | Linux / FreeBSD / *BSD  | Default: UTF-8 (forced).                                |
//! |                         | `linux-batch-extract-setlocale`: follows `LC_CTYPE`.    |
//! |                         | Stock (no batch-extract feature): follows `LC_CTYPE`,   |
//! |                         | caller manages.                                          |
//! | macOS / iOS             | UTF-8 (HFS+ / APFS native; `_APPLE` branch).            |
//! | Windows                 | NTFS UTF-16 wide; rendered per OS `CP_ACP` in console / |
//! |                         | Explorer.                                                |
//!
//! Windows display ↔ codepage mapping (`CP_ACP` follows OS system locale,
//! independent of libc setlocale):
//!
//! - Windows zh-CN: `CP_ACP = CP936` (GBK)
//! - Windows zh-TW: `CP_ACP = CP950` (Big5)
//! - Windows ja-JP: `CP_ACP = CP932` (Shift-JIS)
//! - Windows 10 ≥ 1803 / Windows 11 with the "Beta: Use Unicode UTF-8 for
//!   worldwide language support" toggle enabled: `CP_ACP = CP65001`
//!   (UTF-8). The toggle is OFF by default.

#![warn(missing_docs)]

pub use archive::Archive;
use unrar_ng_sys as native;
mod archive;
pub mod error;
mod locale;
mod pathed;
mod open_archive;
pub use error::UnrarResult;
pub use open_archive::{
    CursorBeforeFile, CursorBeforeHeader, ExtractEvent, ExtractStatus, FileHeader, List, ListSplit,
    OpenArchive, Process, VolumeInfo,
};
