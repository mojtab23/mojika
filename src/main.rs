use std::env::args;

use anyhow::Result;
use log::LevelFilter;

use mojika::app::App;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();

    let mode = args().nth(1).unwrap_or_else(|| "server".to_string());

    let app = App {};
    app.start(&mode).await
}
