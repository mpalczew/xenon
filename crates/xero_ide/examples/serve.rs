//! Manual test harness: start the IDE server, print its port, and echo any
//! commands agents send. Connect with a WebSocket client to exercise it.

fn main() {
    let (tx, rx) = async_channel::unbounded();
    let root = std::env::current_dir().unwrap();
    let server = xero_ide::IdeServer::start(vec![root], tx).expect("start");
    for (key, value) in server.env() {
        println!("{key}={value}");
    }
    std::thread::spawn(move || {
        while let Ok(command) = rx.recv_blocking() {
            match command {
                xero_ide::IdeCommand::OpenFile(path) => {
                    println!("OPEN_FILE {}", path.display());
                }
            }
        }
    });
    std::thread::sleep(std::time::Duration::from_secs(30));
}
