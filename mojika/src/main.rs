use std::error::Error;

use tokio::select;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::Level;

use mojika_discovery::Discovery;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    setup_tracing()?;

    let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);

    tokio::spawn(async move {
        debug!("watcher started");
        let _ = tokio::signal::ctrl_c().await;
        let r = shutdown_tx.send(());
        debug!("got ctrl+C {r:?}");
    });

    let discovery = Discovery::new().await?;

    debug!("Start listening to multicast messages.");
    loop {
        select! {
            msg = discovery.receive_new_message() => {
                debug!("Got: {msg:?}");
            }
            res = shutdown_rx.recv() => {
                debug!("Got {res:?} for shutdown");
                break
            }
            else => {
                debug!("Both channels closed");
                break
            }
        }
    }
    Ok(())
}

fn setup_tracing() -> Result<(), Box<dyn Error>> {
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        // Use a more compact, abbreviated log format
        .compact()
        // Display source code file paths
        .with_file(true)
        // Display source code line numbers
        .with_line_number(true)
        // Display the thread ID an event was recorded on
        .with_thread_ids(true)
        // Don't display the event's target (module path)
        .with_target(false)
        // Build the subscriber
        .finish(); // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
