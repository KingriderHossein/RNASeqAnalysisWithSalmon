mod commands;

fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    match commands::execute(&args) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
