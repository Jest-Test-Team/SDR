use anyhow::{Context, Result, bail};
use axum_server::tls_rustls::RustlsConfig;
use rustls::{
    RootCertStore, ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer},
    server::WebPkiClientVerifier,
};
use std::{fs::File, io::BufReader, path::Path, sync::Arc};

#[derive(Debug, Clone)]
pub struct MtlsConfigPaths {
    pub server_cert: String,
    pub server_key: String,
    pub client_ca: String,
}

pub fn load_mtls_config(paths: &MtlsConfigPaths) -> Result<RustlsConfig> {
    let certs = load_certs(&paths.server_cert)
        .with_context(|| format!("load server certificate {}", paths.server_cert))?;
    let key = load_private_key(&paths.server_key)
        .with_context(|| format!("load server private key {}", paths.server_key))?;
    let client_roots = load_root_store(&paths.client_ca)
        .with_context(|| format!("load client CA {}", paths.client_ca))?;

    let client_verifier = WebPkiClientVerifier::builder(Arc::new(client_roots))
        .build()
        .context("build client certificate verifier")?;

    let provider = rustls::crypto::aws_lc_rs::default_provider();
    let mut server_config = ServerConfig::builder_with_provider(provider.into())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .context("configure TLS 1.3 only")?
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(certs, key)
        .context("configure server certificate")?;
    server_config.alpn_protocols = vec![b"http/1.1".to_vec()];

    Ok(RustlsConfig::from_config(Arc::new(server_config)))
}

fn load_root_store(path: impl AsRef<Path>) -> Result<RootCertStore> {
    let mut roots = RootCertStore::empty();
    for cert in load_certs(path)? {
        roots.add(cert).context("add root certificate")?;
    }
    if roots.is_empty() {
        bail!("no CA certificates found");
    }
    Ok(roots)
}

fn load_certs(path: impl AsRef<Path>) -> Result<Vec<CertificateDer<'static>>> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("parse PEM certificates")?;
    if certs.is_empty() {
        bail!("no certificates found in {}", path.display());
    }
    Ok(certs)
}

fn load_private_key(path: impl AsRef<Path>) -> Result<PrivateKeyDer<'static>> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .context("parse PEM private key")?
        .with_context(|| format!("no private key found in {}", path.display()))
}
