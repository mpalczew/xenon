use std::env;
use xenon_stub::{apply_slot_env, exec_gui, gui_binary};

fn main() {
    apply_slot_env();
    let exe = env::current_exe().unwrap_or_else(|_| "xenon".into());
    let args: Vec<_> = env::args_os().skip(1).collect();
    exec_gui(&gui_binary(&exe), &args);
}
