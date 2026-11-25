#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    let result = if std::env::args_os().any(|arg| arg == "--self-test") {
        copied_md_desktop::self_test()
    } else {
        copied_md_desktop::run()
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("copied.md: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
