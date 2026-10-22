use std::io::Write;
use unrar_ng::Archive;

fn main() -> std::io::Result<()> {
    // Basic args parsing
    // Usage: cargo run --example read_named <archive> <entry-filename-to-print>
    let mut args = std::env::args_os().skip(1);
    let file = args.next().unwrap_or("archive.rar".into());
    let name = args.next().unwrap_or("README.md".into());

    let mut archive = Archive::new(&file).open_for_processing().map_err(std::io::Error::other)?;
    while let Some(header) = archive.read_header().map_err(std::io::Error::other)? {
        archive = if header.entry().filename.as_os_str() == name {
            let mut stdout = std::io::stdout().lock();
            header.read_to(&mut stdout)?;
            return stdout.flush();
        } else {
            header.skip().map_err(std::io::Error::other)?
        }
    }
    eprintln!("error: entry not found");
    std::process::exit(1);
}
