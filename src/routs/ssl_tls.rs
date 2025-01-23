use std::{fs::File, io::BufReader, sync::Arc};

use rustls::ServerConfig;
use rustls_pemfile::certs;

pub fn load_certs(path: &str) -> Vec<rustls::Certificate> {
    let cert_file = File::open(path).expect("Cannot open certificate file");
    let mut reader = BufReader::new(cert_file);
    let certs = certs(&mut reader).expect("Failed to read certificates");
    certs.into_iter().map(rustls::Certificate).collect()
}

pub fn load_private_key(path: &str) -> rustls::PrivateKey {
    let key_file = File::open(path).expect("Failed open private key file");
    let mut reader = BufReader::new(key_file);
    let keys = rustls_pemfile::pkcs8_private_keys(&mut reader).expect("Failed to read private key");
    rustls::PrivateKey(keys[0].clone())
}

pub fn configure_tls(cert_path: &str, key_path: &str) -> Arc<ServerConfig> {
    let certs = load_certs(cert_path);
    let key = load_private_key(key_path);

    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("Failed to configure TLS");

    Arc::new(config)
}
