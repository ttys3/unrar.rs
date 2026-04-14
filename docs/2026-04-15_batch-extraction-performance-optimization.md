# Batch Extraction Performance Optimization

## Background

When using the original `unrar` crate to extract RAR archives containing a large number of small files, extraction performance is dramatically slower than the native `unrar` command-line tool. This document explains the root cause and the optimization implemented in `unrar-ng`.

## The Problem

### Test Environment

- **Test file**: Linux kernel source archive (~94,000 files)
- **Platform**: Linux `/tmp` directory (tmpfs, pure in-memory I/O, no disk bottleneck)
- **CPU**: 12th Gen Intel Core i7

### Performance Comparison

| Tool | Time | Notes |
|------|------|-------|
| `unrar x` (native CLI) | 13s | Reference baseline |
| `unrar` crate (per-file API) | 73s | Original approach |
| `unrar-ng` (`extract_all`) | 13s | Optimized batch API |

**The original per-file API is 5.6x slower than the native CLI.**

After optimization, `unrar-ng` matches native CLI performance exactly.

## Root Cause Analysis

### How the UnRAR DLL API Works

The standard DLL API processes files one at a time:

```c
HANDLE hArc = RAROpenArchiveEx(&data);

while (RARReadHeaderEx(hArc, &header) == 0) {
    RARProcessFile(hArc, RAR_EXTRACT, destPath, NULL);
}

RARCloseArchive(hArc);
```

This design is flexible for selective extraction but introduces a hidden performance penalty during batch extraction.

### The Performance Bottleneck

The key difference lies in how the archive traversal loop is implemented.

**Native CLI (extract.cpp)** — reads headers sequentially:

```cpp
// extract.cpp: ExtractArchive()
while (1)
{
  size_t Size = Arc.ReadHeader();  // reads the next block header directly

  bool Repeat = false;
  if (!ExtractCurrentFile(Arc, Size, Repeat))
    break;
}
```

**DLL API (dll.cpp)** — searches for file headers:

```cpp
// Inside RARReadHeaderEx
Data->HeaderSize = (int)Data->Arc.SearchBlock(HEAD_FILE);
```

### Why `SearchBlock` Is Slow

| Function | Behavior | Cost |
|----------|----------|------|
| `ReadHeader()` | Reads the next block header regardless of type | O(1) per call |
| `SearchBlock(HEAD_FILE)` | Loops through headers until a file header is found | O(n) per call |

`SearchBlock(HEAD_FILE)` internally calls `ReadHeader()` in a loop, skipping non-file blocks (service blocks, end-of-archive markers, etc.):

```cpp
// arcread.cpp
size_t Archive::SearchBlock(HEADER_TYPE HeaderType)
{
  size_t Size, Count = 0;
  while ((Size = ReadHeader()) != 0 &&
         (HeaderType == HEAD_ENDARC || GetHeaderType() != HEAD_ENDARC))
  {
    if ((++Count & 127) == 0)
      Wait();
    if (GetHeaderType() == HeaderType)
      return Size;
    SeekToNext();  // skip non-target block types
  }
  return 0;
}
```

The native CLI delegates all header types to `ExtractCurrentFile()`, which handles:
- `HEAD_FILE`: extract the file
- `HEAD_SERVICE`: process service blocks (e.g., ACL, streams)
- `HEAD_ENDARC`: handle end-of-archive (multi-volume support)

The DLL's per-file API calls `SearchBlock` on every iteration, causing redundant traversal of non-file blocks. For archives with many files, this overhead accumulates significantly.

## The Solution

### New Batch Extraction DLL Function

We added `RARExtractAll` / `RARExtractAllW` to the DLL, using the same traversal loop as the native CLI:

```cpp
// dll.cpp
int PASCAL RARExtractAllW(HANDLE hArcData, wchar *DestPath)
{
  DataSet *Data = (DataSet *)hArcData;
  try
  {
    Data->Cmd.DllError = 0;

    if (DestPath != nullptr && *DestPath != 0)
    {
      Data->Cmd.ExtrPath = DestPath;
      AddEndSlash(Data->Cmd.ExtrPath);
    }
    else
      Data->Cmd.ExtrPath.clear();

    Data->Cmd.Command = L"X";
    Data->Cmd.Test = false;
    Data->Cmd.DllOpMode = RAR_EXTRACT;

    // Same loop pattern as the native CLI
    while (true)
    {
      size_t Size = Data->Arc.ReadHeader();  // direct read, no search

      bool Repeat = false;
      if (!Data->Extract.ExtractCurrentFile(Data->Arc, Size, Repeat))
      {
        if (Repeat)
          continue;  // multi-volume archive: restart
        break;       // end of archive or error
      }
    }
  }
  catch (std::bad_alloc&)
  {
    return ERAR_NO_MEMORY;
  }
  catch (RAR_EXIT ErrCode)
  {
    return Data->Cmd.DllError != 0 ? Data->Cmd.DllError : RarErrorToDll(ErrCode);
  }
  return Data->Cmd.DllError;
}
```

### Rust API

```rust
use unrar::Archive;

fn main() {
    let archive = Archive::new("large_archive.rar")
        .open_for_processing()
        .expect("Failed to open archive");

    // Batch extraction — matches native CLI performance
    archive.extract_all("./output")
        .expect("Failed to extract");
}
```

A callback variant is also available for progress reporting:

```rust
use unrar::{Archive, ExtractEvent};

let archive = Archive::new("archive.rar")
    .open_for_processing()
    .expect("Failed to open archive");

archive.extract_all_with_callback("./output", |event| {
    match event {
        ExtractEvent::Start { filename, .. } => {
            println!("extracting {}...", filename.display());
            true // continue extraction
        }
        ExtractEvent::Ok { .. } => true,
        ExtractEvent::Err { error_code, .. } => {
            eprintln!("error (code: {})", error_code);
            true // continue with remaining files
        }
    }
});
```

## Results

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Extraction time (94k files) | 73s | 13s | **5.6x faster** |
| vs. native CLI | 5.6x slower | Same | Fully matched |

## When to Use

**Use `extract_all` / `extract_all_with_callback` when:**
- Extracting archives with many files (especially small files)
- No per-file filtering or selective extraction is needed
- Performance parity with the native `unrar` CLI is desired

**Use the per-file API (`read_header` + `extract_with_base`) when:**
- Selective extraction based on filename, size, or other criteria is needed
- Per-file metadata inspection is required before extraction
- Custom destination paths per file are needed

## Files Modified

| File | Change |
|------|--------|
| `unrar_sys/vendor/unrar/dll.cpp` | Added `RARExtractAll` and `RARExtractAllW` |
| `unrar_sys/vendor/unrar/dll.hpp` | Added function declarations |
| `unrar_sys/vendor/unrar/dll.def` | Added export symbols (Windows) |
| `unrar_sys/src/lib.rs` | Added FFI bindings |
| `src/open_archive.rs` | Added `extract_all()` and `extract_all_with_callback()` methods |
| `src/pathed/all.rs` | Added `extract_all()` helper |
| `src/pathed/linux.rs` | Added `extract_all()` helper (Linux) |

## References

- [UnRAR source code](https://www.rarlab.com/rar_add.htm)
- [Original unrar.rs repository](https://github.com/muja/unrar.rs)
- [Related Issue](https://github.com/muja/unrar.rs/issues/61)
