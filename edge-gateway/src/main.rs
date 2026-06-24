mod metrics;
mod uart;
mod zmq_pub;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
#[command(name = "edge-gateway", version, about = "UART to ZMQ Bridge")]
struct Args {
    #[arg(long, env = "GW_PORT", default_value = "/dev/ttyUSB1")]
    port: String,
    #[arg(long, env = "GW_BAUD", default_value = "921600")]
    baud: u32,
    #[arg(long, env = "ZMQ_ENDPOINT", default_value = "tcp://0.0.0.0:5556")]
    zmq_endpoint: String,
    #[arg(long, env = "HEALTH_PORT", default_value = "8081")]
    health_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();
    let args = Args::parse();

    info!("Starting Edge Gateway");
    info!("UART: {} @ {}", args.port, args.baud);
    info!("ZMQ PUB: {}", args.zmq_endpoint);
    info!("Health: :{}", args.health_port);

    let (tx, publisher_handle) = zmq_pub::spawn_publisher(args.zmq_endpoint.clone());

    let health_app = metrics::router();
    let health_addr = format!("0.0.0.0:{}", args.health_port);
    let health_listener = tokio::net::TcpListener::bind(&health_addr).await?;
    let health_server = tokio::spawn(async move {
        axum::serve(health_listener, health_app).await.unwrap();
    });

    let uart_port = args.port.clone();
    let uart_handle =
        tokio::spawn(async move { uart::run_uart_reader(uart_port, args.baud, tx).await });

    tokio::select! {
        res = uart_handle => {
            res??;
        }
        res = publisher_handle => {
            res??;
        }
        res = health_server => {
            res?;
        }
    }
    Ok(())
}
