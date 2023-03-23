use std::env::args;
use std::sync::Arc;

use anyhow::Result;
use log::LevelFilter;

use mojika::app::App;

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();

    let mode = args().nth(1).unwrap_or_else(|| "server".to_string());

    let app = Arc::new(App::new(&mode));

    // Arc::try_unwrap(app).unwrap().stop();
    app.start()
}
