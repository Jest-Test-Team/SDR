use anyhow::Result;
use clap::Parser;
use tracing::{info, error};
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
    #[arg(long, env = "HEALTH_PORT", default_value = "8080")]
    health_port: u16,
    #[arg(long, env = "METRICS_PORT", default_value = "9090")]
    metrics_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();
    let args = Args::parse();

    info!("Starting Edge Gateway");
    info!("UART: {} @ {}", args.port, args.baud);
    info!("ZMQ PUB: {}", args.zmq_endpoint);
    info!("Health: :{}", args.health_port);
    info!("Metrics: :{}", args.metrics_port);

    // TODO: Implement UART reading, COBS framing, ZMQ publishing
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}