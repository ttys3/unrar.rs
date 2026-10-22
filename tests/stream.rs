use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
use unrar_ng::error::{Code, When};
use unrar_ng::{Archive, ReadError};

// read_with
#[test]
fn read_with_matches_read_across_entries() {
    // The last entry spans into a missing volume; compare every complete one before it.
    let mut expected = Vec::new();
    let mut archive = Archive::new("data/archive.part1.rar")
        .open_for_processing()
        .unwrap();
    while let Some(header) = archive.read_header().unwrap() {
        match header.read() {
            Ok((data, next)) => {
                expected.push(data);
                archive = next;
            }
            Err(_) => break,
        }
    }
    assert_eq!(expected.len(), 6);

    let mut streamed = Vec::new();
    let mut archive = Archive::new("data/archive.part1.rar")
        .open_for_processing()
        .unwrap();
    for _ in 0..expected.len() {
        let header = archive.read_header().unwrap().unwrap();
        let mut buf = Vec::new();
        archive = header
            .read_with(|chunk| {
                buf.extend_from_slice(chunk);
                Ok::<(), ()>(())
            })
            .unwrap();
        streamed.push(buf);
    }
    assert_eq!(streamed, expected);
}

#[test]
fn read_with_sink_error_is_returned_as_sink() {
    #[derive(Debug, PartialEq)]
    struct Stop(u32);

    let header = Archive::new("data/version.rar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap();
    let mut calls = 0;
    let err = header
        .read_with(|_| {
            calls += 1;
            Err(Stop(42))
        })
        .unwrap_err();
    assert_eq!(calls, 1);
    match err {
        ReadError::Sink(e) => assert_eq!(e, Stop(42)),
        other => panic!("expected Sink, got {other:?}"),
    }
}

#[test]
fn read_with_sink_panic_propagates() {
    #[derive(Debug, PartialEq)]
    struct Boom(u32);

    let header = Archive::new("data/version.rar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap();
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = header.read_with(|_| -> Result<(), ()> { panic::panic_any(Boom(7)) });
    }));
    let payload = result.unwrap_err();
    assert_eq!(payload.downcast_ref::<Boom>(), Some(&Boom(7)));
}

#[test]
fn read_with_missing_password() {
    let header = Archive::new("data/crypted.rar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap();
    let mut received = 0usize;
    let err = header
        .read_with(|chunk| {
            received += chunk.len();
            Ok::<(), ()>(())
        })
        .unwrap_err();
    assert_eq!(received, 0);
    match err {
        ReadError::Unrar(e) => {
            assert_eq!(e.code, Code::MissingPassword);
            assert_eq!(e.when, When::Process);
        }
        other => panic!("expected Unrar, got {other:?}"),
    }

    let mut buf = Vec::new();
    Archive::with_password("data/crypted.rar", "unrar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap()
        .read_with(|chunk| {
            buf.extend_from_slice(chunk);
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(buf, b"target\nCargo.lock\n");
}

#[test]
fn read_with_missing_volume() {
    let mut archive = Archive::new("data/archive.part1.rar")
        .open_for_processing()
        .unwrap();
    let err = loop {
        let header = archive
            .read_header()
            .unwrap()
            .expect("archive ended before hitting the missing volume");
        match header.read_with(|_| Ok::<(), ()>(())) {
            Ok(next) => archive = next,
            Err(e) => break e,
        }
    };
    match err {
        ReadError::Unrar(e) => assert_eq!(e.when, When::Process),
        other => panic!("expected Unrar, got {other:?}"),
    }
}

// read_to
#[test]
fn read_to_vec() {
    let mut buf = Vec::new();
    let next = Archive::new("data/version.rar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap()
        .read_to(&mut buf)
        .unwrap();
    assert_eq!(buf, b"unrar-0.4.0");
    assert!(next.read_header().unwrap().is_none());
}

// ReadError
#[test]
fn read_to_writer_error_kind_preserved() {
    struct Failing;
    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "nope"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let err = Archive::new("data/version.rar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap()
        .read_to(&mut Failing)
        .unwrap_err();
    assert!(matches!(err, ReadError::Sink(_)));
    let io_err: io::Error = err.into();
    assert_eq!(io_err.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(io_err.to_string(), "nope");
}
