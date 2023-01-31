use std::error::Error;

use tokio::sync::broadcast;

use mojika_discovery::Discovery;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    tokio::spawn(async move {
        println!("watcher started");
        let _ = tokio::signal::ctrl_c().await;
        let r = shutdown_tx.send(());
        println!("got ctrl+C {r:?}");
    });

    let mut  discovery = Discovery::new(shutdown_rx).await?;
    discovery.listen_multicast().await;
    Ok(())
}
