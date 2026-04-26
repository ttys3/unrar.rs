//! Regression tests for the public error-code surface in `unrar_ng::error`.

use unrar_ng::error::{Code, UnrarError, When};

#[test]
fn from_maps_known_codes() {
    assert_eq!(Code::from(0), Code::Success);
    assert_eq!(Code::from(10), Code::EndArchive);
    assert_eq!(Code::from(11), Code::NoMemory);
    assert_eq!(Code::from(20), Code::SmallBuf);
    assert_eq!(Code::from(21), Code::Unknown);
    assert_eq!(Code::from(22), Code::MissingPassword);
    assert_eq!(Code::from(23), Code::EReference);
    assert_eq!(Code::from(24), Code::BadPassword);
    assert_eq!(Code::from(25), Code::LargeDict);
}

#[test]
fn from_falls_back_to_unmapped_for_unknown_codes() {
    assert_eq!(Code::from(26), Code::Unmapped(26));
    assert_eq!(Code::from(99), Code::Unmapped(99));
    assert_eq!(Code::from(-1), Code::Unmapped(-1));
}

#[test]
fn unknown_and_unmapped_are_distinct() {
    // ERAR_UNKNOWN (21) is a real DLL code; an unmapped raw value is not.
    assert_ne!(Code::Unknown, Code::Unmapped(21));
    assert_eq!(Code::from(21), Code::Unknown);
    assert_ne!(Code::from(21), Code::Unmapped(21));
}

#[test]
fn display_includes_largedict_message() {
    let err = UnrarError::from(Code::LargeDict, When::Process);
    let rendered = format!("{err}");
    assert!(
        rendered.contains("dictionary too large"),
        "unexpected Display: {rendered}"
    );
}

#[test]
fn display_unmapped_includes_raw_code() {
    let err = UnrarError::from(Code::Unmapped(99), When::Process);
    let rendered = format!("{err}");
    assert!(
        rendered.contains("99"),
        "Unmapped Display should expose raw code, got: {rendered}"
    );
}

#[test]
fn display_does_not_panic_for_any_known_variant() {
    let known = [
        Code::Success,
        Code::EndArchive,
        Code::NoMemory,
        Code::BadData,
        Code::BadArchive,
        Code::UnknownFormat,
        Code::EOpen,
        Code::ECreate,
        Code::EClose,
        Code::ERead,
        Code::EWrite,
        Code::SmallBuf,
        Code::Unknown,
        Code::MissingPassword,
        Code::EReference,
        Code::BadPassword,
        Code::LargeDict,
        Code::Unmapped(42),
    ];
    for code in known {
        for when in [When::Open, When::Read, When::Process] {
            let err = UnrarError::from(code, when);
            let rendered = format!("{err}");
            assert!(!rendered.is_empty(), "empty Display for ({code:?}, {when:?})");
        }
    }
}
