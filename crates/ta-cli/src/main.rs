fn main() {
    let observability_config = match ta_observability::ObservabilityConfig::cli("ta-cli", "warn") {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let _observability = match ta_observability::init(observability_config) {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    let exit_code = match ta_cli::run_env() {
        Ok(()) => 0,
        Err(error) => {
            error.report();
            error.exit_code()
        }
    };

    std::process::exit(exit_code);
}
