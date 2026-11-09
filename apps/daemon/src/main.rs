use miraishin_daemon::{run, RunConfig, RunError};
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(paths) = miraishin_store::Paths::system() else {
        eprintln!("no user data directory available (HOME is not set)");
        return ExitCode::FAILURE;
    };
    let endpoint = match miraishin_ipc::Endpoint::default_for_user() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("no runtime directory for the ipc socket: {e}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("could not start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let config = RunConfig {
        paths,
        endpoint,
        init_logging: true,
        console_logs: true,
        surface: None,
        video_env: None,
        power: None,
        session: None,
    };
    match runtime.block_on(run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        // Código propio para que el autoarranque sepa que no es una falla.
        Err(RunError::AlreadyRunning) => ExitCode::from(3),
        Err(RunError::Other(e)) => {
            tracing::error!(error = %format!("{e:#}"), "daemon failed");
            eprintln!("{e:#}");
            ExitCode::FAILURE
        }
    }
}
