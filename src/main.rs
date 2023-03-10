use std::env::args;

use anyhow::Result;
use log::{debug, info, warn, LevelFilter};
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};

use mojika_new::discovery::Discovery;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    shutdown_watcher(shutdown_tx);

    let mode = args().nth(1).unwrap_or_else(|| "server".to_string());

    let discovery = Discovery::new().await?;

    return if mode == "server" {
        run_server(&discovery, shutdown_rx).await
    } else {
        run_client(&discovery).await
    };
}

async fn run_server(discovery: &Discovery, mut shutdown: Receiver<()>) -> Result<()> {
    info!("Server mode.");
    loop {
        tokio::select! {
            msg = discovery.receive_new_message() =>{
                debug!("Message: {msg}")
            }
            res = shutdown.recv() => {
                debug!("Got {res:?} for shutdown the server");
                break
            }
            else => {
                warn!("Both channels closed");
                break
            }
        }
    }

    Ok(())
}

async fn run_client(discovery: &Discovery) -> Result<()> {
    info!("Client mode.");
    discovery.send_signal().await?;
    Ok(())
}

fn shutdown_watcher(tx: Sender<()>) {
    tokio::spawn(async move {
        debug!("Shutdown watcher started");
        let _ = tokio::signal::ctrl_c().await;
        let r = tx.send(());
        debug!("Got ctrl+C {r:?}");
    });
}
