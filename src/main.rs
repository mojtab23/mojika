use std::sync::Arc;

use anyhow::Result;
use log::LevelFilter;

use mojika::app::App;

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();

    let app = Arc::new(App::new());

    // Arc::try_unwrap(app).unwrap().stop();
    app.start()
}
