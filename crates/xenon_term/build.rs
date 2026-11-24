use std::process::Command;

fn run(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn main() {
    let label = run(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "dev".into());
    println!("cargo:rustc-env=XENON_BUILD={label}");
    println!("cargo:rerun-if-changed=build.rs");
    let paths = [
        run(&["rev-parse", "--git-path", "HEAD"]),
        run(&["symbolic-ref", "-q", "HEAD"])
            .and_then(|head| run(&["rev-parse", "--git-path", &head])),
        run(&["rev-parse", "--git-path", "packed-refs"]),
    ];
    for path in paths.into_iter().flatten() {
        println!("cargo:rerun-if-changed={path}");
    }
}
