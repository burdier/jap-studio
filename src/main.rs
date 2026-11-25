mod audio;
#[cfg(feature = "p2p")]
mod collaboration;
mod instruments;
mod ui;

fn main() {
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
