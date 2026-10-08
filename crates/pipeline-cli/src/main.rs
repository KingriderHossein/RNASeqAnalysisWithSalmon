use pipeline_core::RunState;

fn print_help() {
    println!("RNA-seq Pipeline Manager");
    println!();
    println!("Usage:");
    println!("  rnaseq-pipeline states");
    println!("  rnaseq-pipeline --help");
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("states") => {
            for state in RunState::ALL {
                println!("{state}");
            }
        }
        Some("-h") | Some("--help") | None => print_help(),
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_help();
            std::process::exit(2);
        }
    }
}
