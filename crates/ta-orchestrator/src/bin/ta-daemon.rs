fn main() {
    if let Err(error) = ta_orchestrator::daemon::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
