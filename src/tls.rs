//! One certificate for the application TCP listener, with optional client authentication.
//! Files are loaded at startup; certificate renewal requires a process restart.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::server::WebPkiClientVerifier;
use tokio_rustls::rustls::{RootCertStore, ServerConfig};

pub(crate) fn connection_info(
    connection: &tokio_rustls::rustls::ServerConnection
) -> claviron::http::ext::TlsInfo {
    use claviron::http::ext::{ClientAuthStatus, TlsInfo};
    let verified = connection
        .peer_certificates()
        .is_some_and(|certs| !certs.is_empty());
    TlsInfo {
        protocol_version: connection.protocol_version(),
        cipher_suite: connection
            .negotiated_cipher_suite()
            .map(|suite| suite.suite()),
        client_auth: if verified {
            ClientAuthStatus::Verified
        } else {
            ClientAuthStatus::NotProvided
        },
        client_cert: None,
    }
}

/// The certificate a door serves, and optionally the CA a client must be
/// signed by.
#[derive(Clone, Debug)]
pub struct Tls {
    pub certificate: PathBuf,
    pub key: PathBuf,
    /// Require a client certificate signed by this CA. Without it, clients
    /// are not asked for one.
    pub client_ca: Option<PathBuf>,
}

impl Tls {
    pub fn new(
        certificate: impl Into<PathBuf>,
        key: impl Into<PathBuf>,
    ) -> Self {
        Self { certificate: certificate.into(), key: key.into(), client_ca: None }
    }

    /// Require a client certificate signed by this authority.
    pub fn client_ca(
        mut self,
        path: impl Into<PathBuf>,
    ) -> Self {
        self.client_ca = Some(path.into());
        self
    }

    /// Build the acceptor, reading every file named here.
    pub(crate) fn acceptor(&self) -> Result<TlsAcceptor, String> {
        let chain = certificates(&self.certificate)?;
        let key = key(&self.key)?;

        let config = match &self.client_ca {
            None => ServerConfig::builder().with_no_client_auth(),
            Some(path) => {
                let mut roots = RootCertStore::empty();
                for certificate in certificates(path)? {
                    roots
                        .add(certificate)
                        .map_err(|error| format!("{}: {error}", path.display()))?;
                }
                let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
                    .build()
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                ServerConfig::builder().with_client_cert_verifier(verifier)
            }
        };

        let mut config = config
            .with_single_cert(chain, key)
            .map_err(|error| format!("certificate and key do not match: {error}"))?;
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Ok(TlsAcceptor::from(Arc::new(config)))
    }
}

fn certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>, String> {
    let file = std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let certificates: Result<Vec<_>, _> =
        rustls_pemfile::certs(&mut BufReader::new(file)).collect();
    let certificates = certificates.map_err(|error| format!("{}: {error}", path.display()))?;
    if certificates.is_empty() {
        return Err(format!("{}: no certificate in this file", path.display()));
    }
    Ok(certificates)
}

fn key(path: &Path) -> Result<PrivateKeyDer<'static>, String> {
    let file = std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    rustls_pemfile::private_key(&mut BufReader::new(file))
        .map_err(|error| format!("{}: {error}", path.display()))?
        .ok_or_else(|| format!("{}: no private key in this file", path.display()))
}

#[cfg(test)]
#[path = "../tests/tls.rs"]
pub(crate) mod tests;
