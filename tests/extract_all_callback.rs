//! Integration tests for `OpenArchive::extract_all_with_callback`.
//!
//! These tests stress the cancel-vs-completed signal exposed via
//! `ExtractStatus`, which is the contract behind the vendor patch in
//! `unrar_sys/vendor/patches/0006-feat-ucm-extractfile-callbacks.patch`
//! and the trampoline in `src/open_archive.rs`.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use unrar_ng::{Archive, ExtractEvent, ExtractStatus};

fn temp_dir(tag: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    dir.push(format!("unrar-ng-{tag}-{}-{nonce}", std::process::id()));
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn complete_run_yields_completed_status() {
    let dest = temp_dir("complete");
    let archive = Archive::new("data/version.rar")
        .open_for_processing()
        .expect("open");
    let status = archive
        .extract_all_with_callback(&dest, |_| true)
        .expect("extract_all_with_callback");
    assert_eq!(status, ExtractStatus::Completed);
    cleanup(&dest);
}

#[test]
fn cancel_on_start_yields_cancelled_status() {
    let dest = temp_dir("cancel-start");
    let archive = Archive::new("data/version.rar")
        .open_for_processing()
        .expect("open");
    let status = archive
        .extract_all_with_callback(&dest, |event| !matches!(event, ExtractEvent::Start { .. }))
        .expect("extract_all_with_callback");
    assert_eq!(status, ExtractStatus::Cancelled);
    cleanup(&dest);
}

#[test]
fn cancel_on_ok_yields_cancelled_status() {
    let dest = temp_dir("cancel-ok");
    let archive = Archive::new("data/version.rar")
        .open_for_processing()
        .expect("open");
    let status = archive
        .extract_all_with_callback(&dest, |event| !matches!(event, ExtractEvent::Ok { .. }))
        .expect("extract_all_with_callback");
    assert_eq!(status, ExtractStatus::Cancelled);
    cleanup(&dest);
}

#[test]
fn cancel_on_start_emits_no_ok_for_that_file() {
    let dest = temp_dir("cancel-start-no-ok");
    let archive = Archive::new("data/version.rar")
        .open_for_processing()
        .expect("open");
    let events = RefCell::new(Vec::<&'static str>::new());
    let status = archive
        .extract_all_with_callback(&dest, |event| {
            match &event {
                ExtractEvent::Start { .. } => {
                    events.borrow_mut().push("start");
                    false // cancel before file is actually written
                }
                ExtractEvent::Ok { .. } => {
                    events.borrow_mut().push("ok");
                    true
                }
                ExtractEvent::Err { .. } => {
                    events.borrow_mut().push("err");
                    true
                }
                _ => true,
            }
        })
        .expect("extract_all_with_callback");
    assert_eq!(status, ExtractStatus::Cancelled);
    let observed = events.into_inner();
    assert_eq!(
        observed,
        vec!["start"],
        "after Start cancel, no Ok/Err should follow"
    );
    cleanup(&dest);
}

#[test]
fn ok_callback_runs_for_completed_run() {
    static OK_COUNT: AtomicUsize = AtomicUsize::new(0);
    OK_COUNT.store(0, Ordering::SeqCst);

    let dest = temp_dir("ok-count");
    let archive = Archive::new("data/version.rar")
        .open_for_processing()
        .expect("open");
    let status = archive
        .extract_all_with_callback(&dest, |event| {
            if matches!(event, ExtractEvent::Ok { .. }) {
                OK_COUNT.fetch_add(1, Ordering::SeqCst);
            }
            true
        })
        .expect("extract_all_with_callback");
    assert_eq!(status, ExtractStatus::Completed);
    assert!(
        OK_COUNT.load(Ordering::SeqCst) >= 1,
        "expected at least one Ok event for data/version.rar"
    );
    cleanup(&dest);
}
