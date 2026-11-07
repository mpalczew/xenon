//! Compiles the few icons Adwaita lacks (split right, split down) into a
//! GResource the app registers at start.

use std::{env, path::PathBuf, process::Command};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    println!("cargo:rerun-if-changed=resources");
    let status = Command::new("glib-compile-resources")
        .args(["--sourcedir=resources", "--target"])
        .arg(out.join("xenon.gresource"))
        .arg("resources/xenon.gresource.xml")
        .status()
        .expect("glib-compile-resources (libglib2.0-dev-bin) is needed to build xenon");
    assert!(status.success(), "glib-compile-resources failed");
}
