use std::path::PathBuf;
use std::fs;

#[test]
fn version_list() {
    let mut archive = unrar_ng::Archive::new("data/version.rar")
        .open_for_listing()
        .unwrap();
    assert_eq!(
        archive.next().unwrap().unwrap().filename,
        PathBuf::from("VERSION")
    );
}

#[test]
fn version_cat() {
    let bytes = unrar_ng::Archive::new("data/version.rar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap()
        .read()
        .unwrap()
        .0;
    let s = String::from_utf8(bytes).unwrap();
    assert_eq!(s, "unrar-0.4.0");
}

#[test]
fn extract_to_tempdir() {
    // see https://github.com/muja/unrar.rs/issues/34
    let file = "data/version.rar".to_owned();
    let mut archive = unrar_ng::Archive::new(&file).open_for_processing().expect("open archive");
    let temp_path = tempfile::tempdir().expect("creating tempdir");
    let temp_path = temp_path.path();
    while let Some(header) = archive.read_header().expect("read header") {
        let temp_file_path = temp_path.join(header.entry().filename.as_path());
        archive = header.extract_to(temp_file_path.as_path()).expect("extract_to");
    }
    let entries = std::fs::read_dir(&temp_path).expect("read tempdir").collect::<Result<Vec<_>, _>>().unwrap();
    assert!(entries.len() == 1);
    assert!(entries[0].file_name() == "VERSION");
}

#[test]
fn extract_all_to_tempdir() {
    // Test the batch extraction function
    let file = "data/version.rar".to_owned();
    let archive = unrar_ng::Archive::new(&file).open_for_processing().expect("open archive");
    let temp_path = tempfile::tempdir().expect("creating tempdir");
    let temp_path_ref = temp_path.path();
    
    // Use extract_all for batch extraction
    archive.extract_all(temp_path_ref).expect("extract_all");
    
    // Verify extraction results
    let entries: Vec<_> = fs::read_dir(&temp_path_ref)
        .expect("read tempdir")
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file_name(), "VERSION");
    
    // Verify file content
    let content = fs::read_to_string(temp_path_ref.join("VERSION")).expect("read VERSION");
    assert_eq!(content, "unrar-0.4.0");
}

#[test]
fn extract_all_solid_archive() {
    // Test batch extraction with solid archive
    let file = "data/solid.rar".to_owned();
    let archive = unrar_ng::Archive::new(&file).open_for_processing().expect("open archive");
    let temp_path = tempfile::tempdir().expect("creating tempdir");
    let temp_path_ref = temp_path.path();
    
    // Use extract_all for batch extraction
    archive.extract_all(temp_path_ref).expect("extract_all solid");
    
    // Verify files were extracted
    let entries: Vec<_> = fs::read_dir(&temp_path_ref)
        .expect("read tempdir")
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(!entries.is_empty(), "Expected files to be extracted from solid archive");
}
