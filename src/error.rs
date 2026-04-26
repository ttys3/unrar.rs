#![allow(missing_docs)]

use super::*;
use std::error;
use std::ffi;
use std::fmt;
use std::result::Result;


#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Code {
    Success,
    EndArchive,
    NoMemory,
    BadData,
    BadArchive,
    UnknownFormat,
    EOpen,
    ECreate,
    EClose,
    ERead,
    EWrite,
    SmallBuf,
    Unknown,
    MissingPassword,
    // From the UnRARDLL docs:
    // When attempting to unpack a reference record (see RAR -oi switch),
    // source file for this reference was not found.
    // Entire archive needs to be unpacked to properly create file references.
    // This error is returned when attempting to unpack the reference
    // record without its source file.
    EReference,
    BadPassword,
    LargeDict,
    /// Catches any DLL error code that this Rust enum does not (yet) name.
    /// Carries the raw `int` returned by the DLL so callers can log or match
    /// numerically without panicking when a future UnRAR release adds a new
    /// `ERAR_*`. Distinct from [`Code::Unknown`], which corresponds to the
    /// upstream `ERAR_UNKNOWN(21)` constant.
    Unmapped(i32),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum When {
    Open,
    Read,
    Process,
}

impl Code {
    /// Map a raw DLL error code to a [`Code`] variant.
    ///
    /// Unknown values fall through to [`Code::Unmapped`], which carries
    /// the raw `i32` so callers can log or match numerically without
    /// panicking when a future UnRAR release adds a new `ERAR_*`.
    pub fn from(code: i32) -> Self {
        use Code::*;
        match code {
            native::ERAR_SUCCESS => Success,
            native::ERAR_END_ARCHIVE => EndArchive,
            native::ERAR_NO_MEMORY => NoMemory,
            native::ERAR_BAD_DATA => BadData,
            native::ERAR_BAD_ARCHIVE => BadArchive,
            native::ERAR_UNKNOWN_FORMAT => UnknownFormat,
            native::ERAR_EOPEN => EOpen,
            native::ERAR_ECREATE => ECreate,
            native::ERAR_ECLOSE => EClose,
            native::ERAR_EREAD => ERead,
            native::ERAR_EWRITE => EWrite,
            native::ERAR_SMALL_BUF => SmallBuf,
            native::ERAR_UNKNOWN => Unknown,
            native::ERAR_MISSING_PASSWORD => MissingPassword,
            native::ERAR_EREFERENCE => EReference,
            native::ERAR_BAD_PASSWORD => BadPassword,
            native::ERAR_LARGE_DICT => LargeDict,
            c => Unmapped(c),
        }
    }
}

#[derive(PartialEq)]
pub struct UnrarError {
    pub code: Code,
    pub when: When,
}

impl std::error::Error for UnrarError {}

impl fmt::Debug for UnrarError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}@{:?}", self.code, self.when)?;
        write!(f, " ({})", self)
    }
}

impl fmt::Display for UnrarError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Code::*;
        use self::When::*;
        match (self.code, self.when) {
            (BadData, Open) => write!(f, "Archive header damaged"),
            (BadData, Read) => write!(f, "File header damaged"),
            (BadData, Process) => write!(f, "File CRC error"),
            (UnknownFormat, Open) => write!(f, "Unknown encryption"),
            (EOpen, Process) => write!(f, "Could not open next volume"),
            (UnknownFormat, _) => write!(f, "Unknown archive format"),
            (EOpen, _) => write!(f, "Could not open archive"),
            (NoMemory, _) => write!(f, "Not enough memory"),
            (BadArchive, _) => write!(f, "Not a RAR archive"),
            (ECreate, _) => write!(f, "Could not create file"),
            (EClose, _) => write!(f, "Could not close file"),
            (ERead, _) => write!(f, "Read error"),
            (EWrite, _) => write!(f, "Write error"),
            (SmallBuf, _) => write!(f, "Archive comment was truncated to fit to buffer"),
            (MissingPassword, _) => write!(f, "Password for encrypted archive not specified"),
            (EReference, _) => write!(f, "Cannot open file source for reference record"),
            (BadPassword, _) => write!(f, "Wrong password was specified"),
            (LargeDict, _) => write!(f, "Archive uses a dictionary too large for this build"),
            (Unmapped(c), _) => write!(f, "Unmapped DLL error code: {c}"),
            (Unknown, _) => write!(f, "Unknown error"),
            (EndArchive, _) => write!(f, "Archive end"),
            (Success, _) => write!(f, "Success"),
        }
    }
}

impl UnrarError {
    pub fn from(code: Code, when: When) -> Self {
        UnrarError { code, when }
    }
}

pub type UnrarResult<T> = Result<T, UnrarError>;

#[derive(Debug)]
pub struct NulError(usize);

impl fmt::Display for NulError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nul value found at position: {}", self.0)
    }
}

impl error::Error for NulError {
    fn description(&self) -> &str {
        "nul value found"
    }
}

impl<C> From<widestring::error::ContainsNul<C>> for NulError {
    fn from(e: widestring::error::ContainsNul<C>) -> NulError {
        NulError(e.nul_position())
    }
}

impl From<ffi::NulError> for NulError {
    fn from(e: ffi::NulError) -> NulError {
        NulError(e.nul_position())
    }
}
