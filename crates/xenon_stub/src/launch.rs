use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// GUI Mach-O name beside the stub in `Contents/MacOS`.
pub const GUI_BINARY: &str = "xenon-bin";

/// Path of `xenon-bin` next to the running stub executable.
pub fn gui_binary(stub_exe: &Path) -> PathBuf {
    match stub_exe.parent() {
        Some(dir) => dir.join(GUI_BINARY),
        None => PathBuf::from(GUI_BINARY),
    }
}

/// Replace this process with the GUI. Does not return on success.
pub fn exec_gui(gui: &Path, args: &[OsString]) -> ! {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = Command::new(gui).args(args).exec();
        eprintln!("xenon: failed to exec {}: {error}", gui.display());
        std::process::exit(127);
    }
    #[cfg(not(unix))]
    {
        let _ = (gui, args);
        eprintln!("xenon: stub exec is macOS-only");
        std::process::exit(127);
    }
}
