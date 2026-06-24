use std::net::SocketAddr;

use axum::Router;
use clap::Parser;
use hil_simulator::{AppState, SecureIngestConfig, api};
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
    #[arg(long, env = "SECURE_INGEST_URL")]
    secure_ingest_url: Option<String>,
    #[arg(long, env = "HIL_SIM_TLS_CERT")]
    tls_cert: Option<String>,
    #[arg(long, env = "HIL_SIM_TLS_KEY")]
    tls_key: Option<String>,
    #[arg(long, env = "HIL_SIM_SERVER_CA")]
    server_ca: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let secure_ingest = secure_ingest_config(&args)?;
    let state = AppState::new(args.zmq_endpoint.clone(), secure_ingest.clone());
    info!("HIL simulator (software mode) starting");
    if let Some(config) = &secure_ingest {
        info!("Secure ingest target: {}", config.url);
    } else {
        info!("ZMQ publish target: {}", args.zmq_endpoint);
    }

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

fn secure_ingest_config(args: &Args) -> anyhow::Result<Option<SecureIngestConfig>> {
    match (
        &args.secure_ingest_url,
        &args.tls_cert,
        &args.tls_key,
        &args.server_ca,
    ) {
        (Some(url), Some(client_cert), Some(client_key), Some(server_ca)) => {
            Ok(Some(SecureIngestConfig {
                url: url.clone(),
                client_cert: client_cert.clone(),
                client_key: client_key.clone(),
                server_ca: server_ca.clone(),
            }))
        }
        (None, None, None, None) => Ok(None),
        _ => anyhow::bail!(
            "secure ingest requires SECURE_INGEST_URL, HIL_SIM_TLS_CERT, HIL_SIM_TLS_KEY, and HIL_SIM_SERVER_CA"
        ),
    }
}
