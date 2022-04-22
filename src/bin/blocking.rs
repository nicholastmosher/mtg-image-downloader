use clap::Parser;
use mtg_image_downloader::MtgCli;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let _ = dotenv::dotenv();

    if let None | Some("") = std::env::var("RUST_LOG").ok().as_deref() {
        std::env::set_var("RUST_LOG", "info");
    }

    // tracing_subscriber::fmt()
    //     .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    //     .init();
    // tracing::debug!("Tracing initialized");

    let cli: MtgCli = MtgCli::parse();
    cli.execute()?;

    Ok(())
}
