use std::io::{self, IsTerminal as _};
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdin = io::stdin();
    ExitCode::from(copied_cli::cli::run_system(
        std::env::args_os(),
        stdin.lock(),
        io::stdout().lock(),
        io::stderr().lock(),
        stdin.is_terminal(),
    ))
}
