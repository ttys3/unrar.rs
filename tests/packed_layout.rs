//! Regression tests that exercise the `#[repr(C, packed(1))]` layout of
//! `unrar_ng_sys::HeaderDataEx` by reading fields that sit **after** the first
//! pointer field in the struct.
//!
//! Before the layout fix, the Rust FFI structs used `#[repr(C)]` with natural
//! alignment, while the C++ `RARHeaderDataEx` in `vendor/unrar/dll.hpp` sits
//! inside a `#pragma pack(push, 1)` block. This mismatch moved every Rust
//! field after `comment_buffer` (the first pointer) to a different offset
//! than the DLL writes to, so any Rust code that read a post-divergence
//! field got garbage data.
//!
//! These tests use the low-level `unrar_ng_sys` FFI directly (not the
//! high-level `unrar_ng` crate) to read fields like `hash_type`, `dict_size`,
//! `mtime_low` and `ctime_low` — all of which live past the first pointer
//! and would return garbage on the broken layout. With the packed layout
//! fix, they return values in their documented ranges.

#[cfg(any(target_os = "linux", target_os = "netbsd"))]
use std::ffi::CString;

use unrar_ng_sys as sys;

/// Convert an ASCII path to a null-terminated `wchar_t` buffer. Sufficient
/// for our test fixtures which are all ASCII-only paths under `data/`.
fn to_wchar_buf(path: &str) -> Vec<sys::WCHAR> {
    let mut v: Vec<sys::WCHAR> = path.chars().map(|c| c as i32 as sys::WCHAR).collect();
    v.push(0);
    v
}

/// Open `path` via `RAROpenArchiveEx` in LIST mode, returning the raw
/// handle. `OpenArchiveDataEx::new` has different signatures per platform
/// (`*const c_char` on Linux/NetBSD, `*const wchar_t` elsewhere); this
/// helper papers over that difference.
#[cfg(any(target_os = "linux", target_os = "netbsd"))]
fn open_list(path: &str) -> *const sys::Handle {
    let cpath = CString::new(path).unwrap();
    let mut open_data =
        sys::OpenArchiveDataEx::new(cpath.as_ptr() as *const _, sys::RAR_OM_LIST);
    let handle = unsafe { sys::RAROpenArchiveEx(&mut open_data as *mut _) };
    // `open_data` is `#[repr(C, packed(1))]`, so copy the field to a local
    // before referencing it in `format!` — references into packed structs
    // are forbidden.
    let open_result = open_data.open_result;
    assert!(
        !handle.is_null(),
        "RAROpenArchiveEx failed for {path} (OpenResult={open_result})"
    );
    drop(cpath);
    handle
}

#[cfg(not(any(target_os = "linux", target_os = "netbsd")))]
fn open_list(path: &str) -> *const sys::Handle {
    let wpath = to_wchar_buf(path);
    let mut open_data =
        sys::OpenArchiveDataEx::new(wpath.as_ptr() as *const _, sys::RAR_OM_LIST);
    let handle = unsafe { sys::RAROpenArchiveEx(&mut open_data as *mut _) };
    // See comment on the Linux branch — avoid references into the packed
    // struct by copying the field out first.
    let open_result = open_data.open_result;
    assert!(
        !handle.is_null(),
        "RAROpenArchiveEx failed for {path} (OpenResult={open_result})"
    );
    drop(wpath);
    handle
}

/// Open an archive in LIST mode and read the first header via the raw FFI.
fn read_first_header(path: &str) -> sys::HeaderDataEx {
    let handle = open_list(path);

    let mut header = sys::HeaderDataEx::default();
    let rc = unsafe { sys::RARReadHeaderEx(handle, &mut header as *mut _) };
    assert_eq!(rc, sys::ERAR_SUCCESS, "RARReadHeaderEx returned {rc}");

    let close_rc = unsafe { sys::RARCloseArchive(handle) };
    assert_eq!(close_rc, sys::ERAR_SUCCESS, "RARCloseArchive returned {close_rc}");

    header
}

#[test]
fn hash_type_is_a_valid_enum_value() {
    // `hash_type` sits at offset 10308 on 64-bit linux/macos (after the first
    // pointer field `comment_buffer` + `comment_buffer_size` + `comment_size`
    // + `comment_state` + `dict_size`). In the broken `#[repr(C)]` layout
    // this was at a different Rust offset and would read garbage — often
    // values like 0xFFFFFFFF. After the packed fix it must be one of the
    // three documented enum values: 0 (NONE), 1 (CRC32), 2 (BLAKE2).
    let archives = [
        "data/version.rar",
        "data/unicode-entry.rar",
        "data/solid.rar",
    ];
    for path in archives {
        let header = read_first_header(path);
        let hash_type = header.hash_type;
        assert!(
            matches!(
                hash_type,
                sys::RAR_HASH_NONE | sys::RAR_HASH_CRC32 | sys::RAR_HASH_BLAKE2
            ),
            "{path}: hash_type = {hash_type} is not in {{NONE, CRC32, BLAKE2}} \
             — likely a packed-struct layout regression"
        );
    }
}

#[test]
fn redir_type_is_bounded() {
    // `redir_type` sits at offset 10344 on 64-bit (right after the 32-byte
    // hash array). FSREDIR values are all < 256 in upstream UnRAR. On the
    // broken layout this would read from the middle of a pointer and
    // produce values like 0x5e0_xxxx. After the packed fix it should be a
    // small integer (0 for ordinary files).
    let header = read_first_header("data/version.rar");
    let redir = header.redir_type;
    assert!(
        redir < 16,
        "redir_type = {redir:#x} is too large for an FSREDIR enum — \
         likely a packed-struct layout regression"
    );
}

#[test]
fn time_low_high_fields_are_sane() {
    // The `mtime_low`/`mtime_high` et al. sit at offset 10364..10388 on
    // 64-bit. On the broken layout they'd land on top of pointer bytes and
    // return nonsense. On RAR v5 archives the DLL fills them with FILETIME
    // units (100-ns since 1601). We don't hard-code an expected timestamp
    // because upstream fixtures can change; instead assert that the time
    // is either all-zero (no mtime recorded) or a plausible FILETIME value
    // (after year 2000 = 125_911_584_000_000_000, before year 2100).
    let header = read_first_header("data/unicode-entry.rar");
    let low = header.mtime_low;
    let high = header.mtime_high;
    let filetime = ((high as u64) << 32) | (low as u64);

    const YEAR_2000_FILETIME: u64 = 125_911_584_000_000_000;
    const YEAR_2100_FILETIME: u64 = 157_453_344_000_000_000;

    if filetime != 0 {
        assert!(
            (YEAR_2000_FILETIME..YEAR_2100_FILETIME).contains(&filetime),
            "mtime filetime = {filetime} is not in the year 2000..2100 range \
             — likely a packed-struct layout regression reading garbage bytes \
             (low={low:#x}, high={high:#x})"
        );
    }
}

#[test]
fn open_archive_data_ex_flags_are_bounded() {
    // `OpenArchiveDataEx::flags` is a pre-divergence field so it was always
    // read correctly, but we still assert it as a basic read-back smoke
    // test on the packed struct.
    let handle = open_list("data/version.rar");
    // Re-open via raw FFI so we can inspect open_data after the call.
    let _ = unsafe { sys::RARCloseArchive(handle) };

    #[cfg(any(target_os = "linux", target_os = "netbsd"))]
    let (_keep, mut open_data) = {
        let cpath = CString::new("data/version.rar").unwrap();
        let data = sys::OpenArchiveDataEx::new(cpath.as_ptr() as *const _, sys::RAR_OM_LIST);
        (cpath, data)
    };
    #[cfg(not(any(target_os = "linux", target_os = "netbsd")))]
    let (_keep, mut open_data) = {
        let wpath = to_wchar_buf("data/version.rar");
        let data = sys::OpenArchiveDataEx::new(wpath.as_ptr() as *const _, sys::RAR_OM_LIST);
        (wpath, data)
    };

    let handle2 = unsafe { sys::RAROpenArchiveEx(&mut open_data as *mut _) };
    assert!(!handle2.is_null());
    let flags = open_data.flags;
    // All defined ROADF_* bits fit in 9 bits (0x1ff)
    assert!(flags < 0x1000, "flags = {flags:#x} has unexpected high bits set");
    let _ = unsafe { sys::RARCloseArchive(handle2) };
}

