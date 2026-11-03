//! Private JSON-lines service for the native macOS SwiftUI shell.

fn main() {
    if let Err(error) = nirmoka_app::native::run() {
        eprintln!("nirmoka native service: {error}");
        std::process::exit(1);
    }
}
