//! Optional Rust-side locale initialization for the
//! `linux-batch-extract-setlocale` cargo feature.
//!
//! # Why this exists
//!
//! Stock libunrar's wide ↔ 8-bit filename conversions on Linux / FreeBSD /
//! NetBSD / OpenBSD use [`wcsrtombs`] / [`mbsrtowcs`], both of which consult
//! the process [`LC_CTYPE`]. Under default `C` / `POSIX` locale, every
//! non-ASCII wide character on the output side falls through to a literal
//! `_` byte (see `unicode.cpp:185` in the vendored source). This breaks
//! batch-extract paths (`OpenArchive::extract_all` /
//! `extract_all_with_callback`) which let libunrar internally derive the
//! output filename from the archive's wide-string field. Per-file extract
//! is unaffected because [`crate::pathed`] pre-builds the full destination
//! path on the Rust side and feeds it through libunrar's ANSI `DestName`
//! parameter, which rides a private-use-area round-trip in
//! `CharToWideMap` / `WideToCharMap` (`unicode.cpp:154`+) and is
//! locale-immune.
//!
//! The default `linux-batch-extract-utf8` cargo feature bypasses this
//! whole locale-aware code path via a vendored patch (`vendor/patches/
//! 0007-fix-linux-widetochar-use-utf8.patch`), so this module is not
//! needed and compiles to a no-op stub. For callers who run with
//! `default-features = false` (e.g. to keep on-disk filenames in a
//! legacy host codepage like `zh_CN.GBK` or `ja_JP.eucJP`, mirroring
//! standalone `unrar` CLI behavior under those locales), enabling this
//! crate's `linux-batch-extract-setlocale` feature calls
//! `setlocale(LC_CTYPE, "")` once via [`std::sync::OnceLock`] at the
//! first invocation of one of the batch-extract methods. A fallback
//! ladder (`C.UTF-8` → `C.utf8` → `en_US.UTF-8` → `en_US.utf8`) handles
//! containers / `systemd` units that ship without `LANG` set but with
//! UTF-8 locale data installed (the `glibc` `C.UTF-8` locale has been
//! universally available since 2.13 / 2011).
//!
//! # Scope of the call site
//!
//! Only `extract_all` and `extract_all_with_callback` invoke
//! [`ensure_initialized`]. Listing, reading entries to memory, testing,
//! skipping, and per-file extract methods all skip this — they're
//! either locale-immune by their wstring-direct or private-use-roundtrip
//! code path, or they don't write to disk at all. Restricting the call
//! site keeps the libc-state mutation as narrow as possible.
//!
//! # Cross-platform compilation
//!
//! On macOS / iOS / Windows, this module compiles to a no-op stub
//! regardless of feature flags. The `libc` dependency is declared with a
//! `target = "cfg(all(unix, not(target_vendor = \"apple\")))"` gate in
//! `Cargo.toml`, so it is not pulled into the build on those targets.
//!
//! [`wcsrtombs`]: https://man7.org/linux/man-pages/man3/wcsrtombs.3.html
//! [`mbsrtowcs`]: https://man7.org/linux/man-pages/man3/mbsrtowcs.3.html
//! [`LC_CTYPE`]: https://man7.org/linux/man-pages/man3/setlocale.3.html

#[cfg(all(
    feature = "linux-batch-extract-setlocale",
    unix,
    not(target_vendor = "apple"),
))]
mod imp {
    use std::ffi::CStr;
    use std::sync::OnceLock;

    static INIT: OnceLock<()> = OnceLock::new();

    pub(crate) fn ensure_initialized() {
        INIT.get_or_init(|| unsafe { init() });
    }

    /// Mirrors standalone `unrar` CLI's `rar.cpp:8 setlocale(LC_ALL, "")`,
    /// restricted to `LC_CTYPE` so the host process's strtod / strftime /
    /// etc. are not perturbed.
    ///
    /// # Safety
    ///
    /// `setlocale` mutates process-global libc state and is not thread-safe
    /// against concurrent calls from other threads. Callers who enable
    /// this feature implicitly accept that mutation. `OnceLock` ensures
    /// the init runs exactly once across the lifetime of the process.
    unsafe fn init() {
        // Step 1: skip if the caller has already configured a non-default
        // locale. We only intervene when LC_CTYPE is "C" / "POSIX",
        // i.e. the runtime defaulted because nobody called setlocale
        // explicitly.
        let cur_ptr = libc::setlocale(libc::LC_CTYPE, std::ptr::null());
        if !cur_ptr.is_null() {
            let cur = CStr::from_ptr(cur_ptr).to_bytes();
            if cur != b"C" && cur != b"POSIX" {
                return;
            }
        }

        // Step 2: honor the environment (LANG / LC_*). Identical to what
        // the standalone `unrar` binary does on its first line of main().
        let empty = c"";
        if !libc::setlocale(libc::LC_CTYPE, empty.as_ptr()).is_null() {
            let after_ptr = libc::setlocale(libc::LC_CTYPE, std::ptr::null());
            if !after_ptr.is_null() {
                let after = CStr::from_ptr(after_ptr).to_bytes();
                if after != b"C" && after != b"POSIX" {
                    return;
                }
            }
        }

        // Step 3: fallback ladder. The environment is empty / `C` / `POSIX`,
        // but we want UTF-8 conversion to work. Try the standard names.
        for candidate in [c"C.UTF-8", c"C.utf8", c"en_US.UTF-8", c"en_US.utf8"] {
            if !libc::setlocale(libc::LC_CTYPE, candidate.as_ptr()).is_null() {
                return;
            }
        }
        // All candidates failed (extremely minimal image with no UTF-8
        // locale data installed). Leave LC_CTYPE as-is — behavior matches
        // standalone `unrar` CLI in this case (mangled `_` for non-ASCII).
    }
}

#[cfg(not(all(
    feature = "linux-batch-extract-setlocale",
    unix,
    not(target_vendor = "apple"),
)))]
mod imp {
    /// No-op stub. Compiled on macOS / iOS / Windows targets, on Linux/BSD
    /// when the `linux-batch-extract-setlocale` cargo feature is disabled,
    /// and on every target when the feature is disabled. The corresponding
    /// `libc` optional dep is target-gated out so this stub introduces zero
    /// runtime cost or dependency footprint.
    #[inline(always)]
    pub(crate) fn ensure_initialized() {}
}

pub(crate) use imp::ensure_initialized;
