use anyhow::Result;
use clap::Parser;
use tracing::{info, error};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
#[command(name = "control-plane", version, about = "Telemetry Processing & Rules Engine")]
struct Args {
    #[arg(long, env = "ZMQ_ENDPOINT", default_value = "tcp://127.0.0.1:5556")]
    zmq_endpoint: String,
    #[arg(long, env = "DB_PATH", default_value = "/data/telemetry.db")]
    db_path: String,
    #[arg(long, env = "HEALTH_PORT", default_value = "8080")]
    health_port: u16,
    #[arg(long, env = "METRICS_PORT", default_value = "9090")]
    metrics_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();
    let args = Args::parse();

    info!("Starting Control Plane");
    info!("ZMQ SUB: {}", args.zmq_endpoint);
    info!("DB: {}", args.db_path);
    info!("Health: :{}", args.health_port);
    info!("Metrics: :{}", args.metrics_port);

    // TODO: Implement ZMQ subscription, decoding, rules engine, storage
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}