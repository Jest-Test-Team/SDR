use anyhow::Result;
use clap::Parser;
use control_plane::{live::LiveBus, store::TelemetryStore, subscriber};
use protocol::ReplayGuard;
use std::sync::{Arc, Mutex};
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
#[command(
    name = "control-plane",
    version,
    about = "Telemetry Processing & Rules Engine"
)]
struct Args {
    #[arg(long, env = "ZMQ_ENDPOINT", default_value = "tcp://127.0.0.1:5556")]
    zmq_endpoint: String,
    #[arg(long, env = "DB_PATH", default_value = "./data/telemetry.db")]
    db_path: String,
    #[arg(long, env = "HEALTH_PORT", default_value = "8080")]
    health_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();
    let args = Args::parse();

    info!("Starting Control Plane");
    info!("ZMQ SUB: {}", args.zmq_endpoint);
    info!("DB: {}", args.db_path);
    info!("Health: :{}", args.health_port);

    let store = Arc::new(TelemetryStore::open(&args.db_path)?);
    let replay = Arc::new(Mutex::new(ReplayGuard::new()));
    let live_bus = LiveBus::new(200);

    let app = control_plane::metrics::router().merge(control_plane::live::router(live_bus.clone()));
    let health_addr = format!("0.0.0.0:{}", args.health_port);
    let health_listener = tokio::net::TcpListener::bind(&health_addr).await?;
    let health_server = tokio::spawn(async move {
        axum::serve(health_listener, app).await.unwrap();
    });

    let subscriber = tokio::spawn(subscriber::run_subscriber(
        args.zmq_endpoint,
        replay,
        store,
        live_bus,
    ));

    tokio::select! {
        res = subscriber => {
            res??;
        }
        res = health_server => {
            res?;
        }
    }
    Ok(())
}
