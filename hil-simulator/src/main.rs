use std::net::SocketAddr;

use axum::Router;
use clap::Parser;
use hil_simulator::{AppState, api};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "hil-simulator", about = "ESP32-S3 to SDR HIL simulator API")]
struct Args {
    #[arg(long, env = "HIL_PORT", default_value = "8090")]
    port: u16,
    #[arg(long, env = "ZMQ_ENDPOINT", default_value = "tcp://127.0.0.1:5556")]
    zmq_endpoint: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let state = AppState::new(args.zmq_endpoint.clone());
    info!("HIL simulator (software mode) starting");
    info!("ZMQ publish target: {}", args.zmq_endpoint);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new().merge(api::router(state)).layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
