//! TLS 1.3 with RFC 7250 raw public keys: each side presents its bare
//! Ed25519 device key, and the only trust decision is "is this key on my
//! roster, unrevoked". No certificates, names, CAs or validity dates.

use std::collections::BTreeSet;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::AlwaysResolvesClientRawPublicKeys;
use rustls::crypto::{ring, verify_tls13_signature_with_raw_key, CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, SubjectPublicKeyInfoDer, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::server::AlwaysResolvesServerRawPublicKeys;
use rustls::sign::CertifiedKey;
use rustls::{ClientConfig, DigitallySignedStruct, DistinguishedName, Error, ServerConfig, SignatureScheme};
use wugs_auth::{DeviceKey, PublicKey};

/// ALPN carries the session version. A peer offering only another version
/// fails the handshake; one offering none completes it, and wugs-net then
/// refuses the session before any frame, so frames are never misread.
pub const ALPN: &[u8] = b"wugs/3";

/// rustls wants a server name even when nothing checks it.
pub fn server_name() -> ServerName<'static> {
    ServerName::try_from("wugs.invalid").expect("static name")
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(ring::default_provider())
}

fn certified(key: &DeviceKey, provider: &CryptoProvider) -> Result<Arc<CertifiedKey>, String> {
    let der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.pkcs8_der().to_vec()));
    let signer = provider.key_provider.load_private_key(der).map_err(|e| format!("device key: {e}"))?;
    Ok(Arc::new(CertifiedKey::new(vec![CertificateDer::from(key.public().spki_der())], signer)))
}

/// Accepts a peer whose raw key is in `allowed` (members and guests that
/// are not revoked, snapshotted from the roster when the session starts).
#[derive(Debug)]
struct RosterVerifier {
    allowed: BTreeSet<PublicKey>,
    algs: WebPkiSupportedAlgorithms,
}

impl RosterVerifier {
    fn check(&self, end_entity: &CertificateDer<'_>) -> Result<(), Error> {
        let key = PublicKey::from_spki_der(end_entity.as_ref()).map_err(|e| Error::General(e.to_string()))?;
        if self.allowed.contains(&key) {
            Ok(())
        } else {
            Err(Error::General(format!("device key {key} is not on this project's roster")))
        }
    }

    fn signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature_with_raw_key(message, &SubjectPublicKeyInfoDer::from(cert.as_ref()), dss, &self.algs)
    }
}

impl ServerCertVerifier for RosterVerifier {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, Error> {
        self.check(end_entity).map(|_| ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        Err(Error::General("TLS 1.2 is not offered".into()))
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        self.signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }

    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

impl ClientCertVerifier for RosterVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(&self, end_entity: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: UnixTime) -> Result<ClientCertVerified, Error> {
        self.check(end_entity).map(|_| ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        Err(Error::General("TLS 1.2 is not offered".into()))
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        self.signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }

    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

fn verifier(allowed: BTreeSet<PublicKey>, provider: &CryptoProvider) -> Arc<RosterVerifier> {
    Arc::new(RosterVerifier { allowed, algs: provider.signature_verification_algorithms })
}

pub fn server_config(key: &DeviceKey, allowed: BTreeSet<PublicKey>) -> Result<Arc<ServerConfig>, String> {
    let p = provider();
    let mut c = ServerConfig::builder_with_provider(p.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| e.to_string())?
        .with_client_cert_verifier(verifier(allowed, &p))
        .with_cert_resolver(Arc::new(AlwaysResolvesServerRawPublicKeys::new(certified(key, &p)?)));
    c.alpn_protocols = vec![ALPN.to_vec()];
    Ok(Arc::new(c))
}

pub fn client_config(key: &DeviceKey, allowed: BTreeSet<PublicKey>) -> Result<Arc<ClientConfig>, String> {
    let p = provider();
    let mut c = ClientConfig::builder_with_provider(p.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(verifier(allowed, &p))
        .with_client_cert_resolver(Arc::new(AlwaysResolvesClientRawPublicKeys::new(certified(key, &p)?)));
    c.alpn_protocols = vec![ALPN.to_vec()];
    Ok(Arc::new(c))
}

/// The peer's device key, once the handshake has verified it.
pub fn peer_key(certs: Option<&[CertificateDer<'_>]>) -> Result<PublicKey, String> {
    let der = certs.and_then(|c| c.first()).ok_or("peer presented no key")?;
    PublicKey::from_spki_der(der.as_ref())
}
