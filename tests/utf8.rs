use std::path::PathBuf;
use unrar_ng::Archive;

#[test]
fn unicode_list() {
    let mut entries = Archive::new("data/unicode.rar").open_for_listing().unwrap();
    assert_eq!(entries.next().unwrap().unwrap().filename, PathBuf::from("te…―st✌"));
}

#[test]
fn unicode_file() {
    let mut entries = Archive::new("data/unicodefilename❤️.rar").open_for_listing().unwrap();
    assert_eq!(entries.next().unwrap().unwrap().filename, PathBuf::from(".gitignore"));
}

#[test]
fn unicode_extract_to() {
    let parent = tempfile::tempdir().unwrap();
    let unicode_file = parent.path().join("unicodefilename❤️.txt");
    let archive = Archive::new("data/version.rar").open_for_processing().unwrap();
    let archive = archive.read_header().unwrap().unwrap();
    archive.extract_to(&unicode_file).expect("extraction failed");
    assert_eq!("unrar-0.4.0", std::fs::read_to_string(unicode_file).expect("read failed"));
}

#[test]
fn extract_with_unicode_base() {
    let parent = tempfile::tempdir().unwrap();
    let unicode_dir = parent.path().join("unicodefilename❤️");
    std::fs::create_dir(&unicode_dir).expect("create dir");
    let archive = Archive::new("data/version.rar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap();
    archive.extract_with_base(&unicode_dir).expect("extraction failed");
    assert_eq!(
        "unrar-0.4.0",
        std::fs::read_to_string(unicode_dir.join("VERSION")).expect("read failed")
    );
}

#[test]
fn unicode_entry() {
    let archive = Archive::new("data/unicode-entry.rar").open_for_listing().unwrap();
    let archive = archive.read_header().unwrap().unwrap();
    assert_eq!(archive.entry().filename.as_os_str(), "unicodefilename❤️.txt");
}

#[test]
fn unicode_entry_process_mode() {
    let archive = Archive::new("data/unicode-entry.rar").open_for_processing().unwrap();
    let archive = archive.read_header().unwrap().unwrap();
    assert_eq!(archive.entry().filename.as_os_str(), "unicodefilename❤️.txt");
    assert_eq!(&String::from_utf8(archive.read().unwrap().0).unwrap(), "foobar\n");
}

#[test]
fn unicode_entry_extract() {
    let parent = tempfile::tempdir().unwrap();
    let archive = Archive::new("data/unicode-entry.rar").open_for_processing().unwrap();
    let archive = archive.read_header().unwrap().unwrap();
    archive.extract_with_base(&parent).expect("extract");
    let entries = std::fs::read_dir(&parent).expect("read_dir").collect::<Result<Vec<_>, _>>().expect("read_dir[0]");
    assert_eq!(entries.len(), 1);
    assert_eq!(&entries[0].file_name(), "unicodefilename❤️.txt");
}

// Regression network for the Linux/BSD batch-extract locale bug.
//
// libunrar's `RARExtractAll(W)` path lets libunrar internally derive the
// output filename from the archive entry's wstring (real CJK codepoints
// like U+2764 for ❤). On non-Apple Unix, `File::Create` then converts
// that wstring back to a char* via `WideToChar` (`unicode.cpp:215`).
// Under `LC_CTYPE=C/POSIX`, `wcsrtombs` fails on non-ASCII codepoints,
// falls back to `WideToCharMap`, and writes literal `_` (or truncates
// the rest of the name) for every codepoint outside the 0xE000-0xE0FF
// private-use round-trip window — which the real CJK / heart codepoints
// are not.
//
// The default `linux-batch-extract-utf8` cargo feature (ON by default)
// routes `WideToChar` through `WideToUtf` instead via vendor patch 0007,
// fixing this. The `linux-batch-extract-setlocale` cargo feature provides
// an alternative Rust-side `setlocale(LC_CTYPE, "")` lazy init for callers
// who run `default-features = false` and want CLI-equivalent locale
// behavior.
//
// Empirically verified to FAIL on Linux + `LC_ALL=C` against unpatched
// 0.7.5 (filename came back as just "unicodefilename", with `❤️.txt`
// mangled and truncated). Should PASS under any CI-supported config:
//   - Default features on Linux/macOS/Windows under any LC.
//   - `--no-default-features --features linux-batch-extract-setlocale`
//     on Linux/macOS/Windows under any LC (fallback ladder hits C.UTF-8).
//
// NOT expected to pass under `--no-default-features` alone (no feature)
// + Linux + `LC_ALL=C`: that's caller-managed mode, by design.

#[test]
fn extract_all_preserves_non_ascii_filenames() {
    let parent = tempfile::tempdir().unwrap();
    Archive::new("data/unicode-entry.rar")
        .open_for_processing()
        .unwrap()
        .extract_all(&parent)
        .expect("extract_all");
    let entries: Vec<_> = std::fs::read_dir(&parent)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    let name = entries[0].to_string_lossy();
    assert!(
        name.contains('\u{2764}'),
        "extract_all mangled the filename: {name:?}"
    );
}

#[test]
fn extract_all_with_callback_preserves_non_ascii_filenames() {
    let parent = tempfile::tempdir().unwrap();
    Archive::new("data/unicode-entry.rar")
        .open_for_processing()
        .unwrap()
        .extract_all_with_callback(&parent, |_| true)
        .expect("extract_all_with_callback");
    let entries: Vec<_> = std::fs::read_dir(&parent)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    let name = entries[0].to_string_lossy();
    assert!(
        name.contains('\u{2764}'),
        "extract_all_with_callback mangled the filename: {name:?}"
    );
}
