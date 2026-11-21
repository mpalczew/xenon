//! Dev-only entry for the spike: runs the terminal service.
//! Usage: `xenon-term [SOCKET]`.

fn main() -> std::io::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map_or_else(xenon_term::default_socket, std::path::PathBuf::from);
    xenon_term::run_service(&path)
}
