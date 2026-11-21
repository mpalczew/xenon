//! The terminal service. systemd's user manager starts it through `xenon-term.socket`;
//! with `XENON_TERM_SOCKET` set it binds that path itself.

fn main() -> std::io::Result<()> {
    xenon_term::run_service()
}
