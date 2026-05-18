mod cli;
mod server;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    cli::run()?;

    Ok(())
}
