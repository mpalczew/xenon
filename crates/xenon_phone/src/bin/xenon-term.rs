use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args[..] {
        ["hold", id] => match id.parse() {
            Ok(id) => xenon_term::run_holder(id),
            Err(error) => Err(std::io::Error::other(format!("bad terminal id: {error}"))),
        },
        ["phone"] => xenon_phone::run(),
        ["end-all"] => xenon_term::end_all(),
        ["version"] => {
            println!("{}", xenon_term::BUILD);
            Ok(())
        }
        _ => {
            eprintln!("usage: xenon-term hold <id> | phone | end-all | version");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xenon-term: {error}");
            ExitCode::FAILURE
        }
    }
}
