use cb_common::{config::StartSignerConfig, utils::initialize_tracing_log};
use cb_signer::service::SigningService;
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // set default backtrace unless provided
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let config = StartSignerConfig::load_from_env()?;
    initialize_tracing_log(config.logs_settings.clone());
    SigningService::run(config).await
}
