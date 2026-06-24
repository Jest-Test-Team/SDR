use anyhow::Result;
use clap::Parser;
use control_plane::{
    ingest::IngestState,
    live::LiveBus,
    store::TelemetryStore,
    subscriber,
    tls::{MtlsConfigPaths, load_mtls_config},
};
use protocol::ReplayGuard;
use std::net::SocketAddr;
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
    #[arg(long, env = "SECURE_INGEST_ONLY", default_value_t = false)]
    secure_ingest_only: bool,
    #[arg(long, env = "CONTROL_PLANE_TLS_CERT")]
    tls_cert: Option<String>,
    #[arg(long, env = "CONTROL_PLANE_TLS_KEY")]
    tls_key: Option<String>,
    #[arg(long, env = "CONTROL_PLANE_CLIENT_CA")]
    client_ca: Option<String>,
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
    let tls_paths = mtls_paths(&args)?;

    let ingest_state = IngestState {
        replay: replay.clone(),
        store: store.clone(),
        live: live_bus.clone(),
    };
    let app = control_plane::metrics::router()
        .merge(control_plane::live::router(live_bus.clone()))
        .merge(control_plane::ingest::router(ingest_state));
    let health_addr = SocketAddr::from(([0, 0, 0, 0], args.health_port));
    let tls_enabled = tls_paths.is_some();
    let health_server = tokio::spawn(async move {
        if let Some(paths) = tls_paths {
            let config = load_mtls_config(&paths).expect("load mTLS configuration");
            info!("HTTP API using TLS 1.3 with required client certificates");
            axum_server::bind_rustls(health_addr, config)
                .serve(app.into_make_service())
                .await
                .unwrap();
        } else {
            let listener = tokio::net::TcpListener::bind(health_addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        }
    });

    if args.secure_ingest_only {
        info!("Secure ingest only: ZMQ subscriber disabled");
        health_server.await?;
        return Ok(());
    }

    if tls_enabled {
        info!("Secure ingest enabled; ZMQ subscriber remains enabled for local/dev sidecar mode");
    }

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

fn mtls_paths(args: &Args) -> Result<Option<MtlsConfigPaths>> {
    match (&args.tls_cert, &args.tls_key, &args.client_ca) {
        (Some(server_cert), Some(server_key), Some(client_ca)) => Ok(Some(MtlsConfigPaths {
            server_cert: server_cert.clone(),
            server_key: server_key.clone(),
            client_ca: client_ca.clone(),
        })),
        (None, None, None) if !args.secure_ingest_only => Ok(None),
        _ => anyhow::bail!(
            "mTLS ingest requires CONTROL_PLANE_TLS_CERT, CONTROL_PLANE_TLS_KEY, and CONTROL_PLANE_CLIENT_CA"
        ),
    }
}
