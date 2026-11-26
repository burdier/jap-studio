mod audio;
#[cfg(feature = "p2p")]
mod collaboration;
mod instruments;
mod mcp;
mod ui;

fn main() {
    if std::env::args_os().nth(1).is_some_and(|arg| arg == "--mcp") {
        if let Err(error) = mcp::run_stdio_server() {
            eprintln!("MCP: {error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(feature = "p2p")]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--collab")
    {
        if let Err(error) = collaboration::run(std::env::args_os().skip(2).collect()) {
            eprintln!("Colaboración: {error}");
            std::process::exit(1);
        }
        return;
    }

    ui::run();
}
