use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use dashmap::DashMap;
use instant_acme::{
    Account, AccountCredentials, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt,
    NewAccount, NewOrder, RetryPolicy,
};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use tokio_rustls::TlsAcceptor;

use crate::config::TlsConfig;

#[derive(Clone)]
pub struct CertStore {
    certs: Arc<DashMap<String, Arc<CertifiedKey>>>,
}

impl std::fmt::Debug for CertStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertStore")
            .field("domains", &self.certs.len())
            .finish()
    }
}

impl CertStore {
    fn new() -> Self {
        Self {
            certs: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, domain: String, key: Arc<CertifiedKey>) {
        self.certs.insert(domain, key);
    }

    fn load_from_disk(&self, cert_dir: &str) -> Result<()> {
        let dir = Path::new(cert_dir);
        if !dir.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir).context("failed to read cert dir")?;
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let domain = entry.file_name().to_string_lossy().to_string();
            let cert_path = entry.path().join("cert.pem");
            let key_path = entry.path().join("key.pem");

            if cert_path.exists() && key_path.exists() {
                match load_certified_key(&cert_path, &key_path) {
                    Ok(key) => {
                        tracing::info!("loaded certificate for {domain}");
                        self.certs.insert(domain, Arc::new(key));
                    }
                    Err(e) => tracing::warn!("failed to load cert for {domain}: {e}"),
                }
            }
        }

        Ok(())
    }
}

impl ResolvesServerCert for CertStore {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let sni = client_hello.server_name()?;
        self.certs.get(sni).map(|entry| entry.value().clone())
    }
}

#[derive(Clone)]
pub struct ChallengeStore {
    tokens: Arc<DashMap<String, String>>,
}

impl ChallengeStore {
    fn new() -> Self {
        Self {
            tokens: Arc::new(DashMap::new()),
        }
    }

    pub fn get(&self, token: &str) -> Option<String> {
        self.tokens.get(token).map(|v| v.value().clone())
    }

    fn insert(&self, token: String, key_authorization: String) {
        self.tokens.insert(token, key_authorization);
    }

    fn remove(&self, token: &str) {
        self.tokens.remove(token);
    }
}

pub async fn provision_app(
    domain: &str,
    config: &TlsConfig,
    challenge_store: &ChallengeStore,
    cert_store: &CertStore,
) -> Result<()> {
    let domain_dir = PathBuf::from(&config.cert_dir).join(domain);
    let cert_path = domain_dir.join("cert.pem");
    let key_path = domain_dir.join("key.pem");

    if cert_path.exists() && key_path.exists() {
        let cert_pem = std::fs::read_to_string(&cert_path)?;
        if !is_expiring_soon(&cert_pem) {
            return Ok(());
        }
    }

    tracing::info!("provisioning certificate for {domain}");

    let account = load_or_create_account(config).await?;
    let identifiers = vec![Identifier::Dns(domain.to_string())];
    let mut order = account
        .new_order(&NewOrder::new(&identifiers))
        .await
        .context("failed to create acme order")?;

    let mut tokens_to_clean = Vec::new();

    let mut authorizations = order.authorizations();
    while let Some(result) = authorizations.next().await {
        let mut authz = result.context("failed to get authorization")?;

        if authz.status == AuthorizationStatus::Valid {
            continue;
        }

        let mut challenge = authz
            .challenge(ChallengeType::Http01)
            .context("http-01 challenge not available")?;

        let token = challenge.token.clone();
        let key_auth = challenge.key_authorization().as_str().to_string();

        challenge_store.insert(token.clone(), key_auth);
        tokens_to_clean.push(token);

        challenge
            .set_ready()
            .await
            .context("failed to set challenge ready")?;
    }

    for token in &tokens_to_clean {
        challenge_store.remove(token);
    }

    order
        .poll_ready(&RetryPolicy::default())
        .await
        .context("order did not become ready")?;

    let private_key_pem = order.finalize().await.context("failed to finalize order")?;

    let cert_chain_pem = order
        .poll_certificate(&RetryPolicy::default())
        .await
        .context("failed to get certificate")?;

    std::fs::create_dir_all(&domain_dir)
        .with_context(|| format!("failed to create cert dir for {domain}"))?;
    std::fs::write(&cert_path, &cert_chain_pem).context("failed to write cert file")?;
    std::fs::write(&key_path, &private_key_pem).context("failed to write key file")?;

    let certified_key = load_certified_key(&cert_path, &key_path)?;
    cert_store.insert(domain.to_string(), Arc::new(certified_key));

    tracing::info!("certificate provisioned for {domain}");
    Ok(())
}

pub async fn init(
    config: &TlsConfig,
    domain: &str,
) -> Result<(TlsAcceptor, ChallengeStore, CertStore)> {
    std::fs::create_dir_all(&config.cert_dir)
        .with_context(|| format!("failed to create cert dir: {}", config.cert_dir))?;

    let cert_store = CertStore::new();
    cert_store.load_from_disk(&config.cert_dir)?;

    let challenge_store = ChallengeStore::new();

    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(cert_store.clone()));

    let acceptor = TlsAcceptor::from(Arc::new(tls_config));

    tokio::spawn(renewal_loop(
        config.clone(),
        domain.to_string(),
        challenge_store.clone(),
        cert_store.clone(),
    ));

    Ok((acceptor, challenge_store, cert_store))
}

async fn renewal_loop(
    config: TlsConfig,
    domain: String,
    challenge_store: ChallengeStore,
    cert_store: CertStore,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(12 * 60 * 60));
    interval.tick().await;

    loop {
        interval.tick().await;

        let cert_dir = Path::new(&config.cert_dir);
        let entries = match std::fs::read_dir(cert_dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("failed to read cert dir for renewal: {e}");
                continue;
            }
        };

        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                continue;
            }

            let app_domain = entry.file_name().to_string_lossy().to_string();
            let cert_path = entry.path().join("cert.pem");

            let needs_renewal = match std::fs::read_to_string(&cert_path) {
                Ok(pem) => is_expiring_soon(&pem),
                Err(_) => continue,
            };

            if !needs_renewal {
                continue;
            }

            let fqdn = format!("{app_domain}.{domain}");
            tracing::info!("renewing certificate for {fqdn}");

            if let Err(e) = provision_app(&fqdn, &config, &challenge_store, &cert_store).await {
                tracing::error!("certificate renewal failed for {fqdn}: {e}");
            }
        }
    }
}

async fn load_or_create_account(config: &TlsConfig) -> Result<Account> {
    let credentials_path = PathBuf::from(&config.cert_dir).join("account.json");

    let builder = Account::builder().context("failed to create account builder")?;

    if credentials_path.exists() {
        let json = std::fs::read_to_string(&credentials_path)
            .context("failed to read account credentials")?;
        let credentials: AccountCredentials =
            serde_json::from_str(&json).context("failed to parse account credentials")?;
        return builder
            .from_credentials(credentials)
            .await
            .context("failed to load acme account");
    }

    let contact = format!("mailto:{}", config.acme_email);
    let (account, credentials) = builder
        .create(
            &NewAccount {
                terms_of_service_agreed: true,
                only_return_existing: false,
                contact: &[&contact],
            },
            LetsEncrypt::Production.url().to_string(),
            None,
        )
        .await
        .context("failed to create acme account")?;

    let json = serde_json::to_string_pretty(&credentials)?;
    std::fs::write(&credentials_path, json).context("failed to save account credentials")?;

    Ok(account)
}

fn load_certified_key(cert_path: &Path, key_path: &Path) -> Result<CertifiedKey> {
    let cert_pem = std::fs::read(cert_path).context("failed to read cert file")?;
    let key_pem = std::fs::read(key_path).context("failed to read key file")?;

    let certs: Vec<_> = rustls_pemfile::certs(&mut &cert_pem[..])
        .collect::<std::result::Result<_, _>>()
        .context("failed to parse cert pem")?;

    let key = rustls_pemfile::private_key(&mut &key_pem[..])
        .context("failed to parse key pem")?
        .context("no private key found in pem")?;

    let signing_key =
        rustls::crypto::ring::sign::any_supported_type(&key).context("unsupported key type")?;

    Ok(CertifiedKey::new(certs, signing_key))
}

fn is_expiring_soon(pem: &str) -> bool {
    use rustls_pemfile::certs;

    let mut reader = pem.as_bytes();
    let cert = match certs(&mut reader).next() {
        Some(Ok(c)) => c,
        _ => return true,
    };

    let parsed = match x509_parser_lite(&cert) {
        Some(not_after) => not_after,
        None => return true,
    };

    let now = std::time::SystemTime::now();
    let thirty_days = std::time::Duration::from_secs(30 * 24 * 60 * 60);

    match now.checked_add(thirty_days) {
        Some(threshold) => threshold > parsed,
        None => true,
    }
}

fn x509_parser_lite(der: &[u8]) -> Option<std::time::SystemTime> {
    let outer = asn1_sequence(der)?;
    let tbs = asn1_sequence(outer)?;

    let mut pos = 0;
    if tbs.get(pos)? & 0xe0 == 0xa0 {
        let (_, consumed) = asn1_element(&tbs[pos..])?;
        pos += consumed;
    }
    let (_, consumed) = asn1_element(&tbs[pos..])?;
    pos += consumed;
    let (_, consumed) = asn1_element(&tbs[pos..])?;
    pos += consumed;
    let (_, consumed) = asn1_element(&tbs[pos..])?;
    pos += consumed;
    let (validity_bytes, _) = asn1_element(&tbs[pos..])?;
    let validity = asn1_contents(validity_bytes)?;

    let (_, consumed) = asn1_element(validity)?;
    let (not_after_elem, _) = asn1_element(&validity[consumed..])?;
    let not_after = asn1_contents(not_after_elem)?;

    parse_asn1_time(not_after_elem.first().copied()?, not_after)
}

fn asn1_sequence(der: &[u8]) -> Option<&[u8]> {
    if der.first()? != &0x30 {
        return None;
    }
    asn1_contents(der)
}

fn asn1_element(der: &[u8]) -> Option<(&[u8], usize)> {
    if der.is_empty() {
        return None;
    }
    let (content_start, length) = asn1_length(&der[1..])?;
    let total = 1 + content_start + length;
    Some((&der[..total], total))
}

fn asn1_contents(der: &[u8]) -> Option<&[u8]> {
    let (content_start, length) = asn1_length(&der[1..])?;
    let start = 1 + content_start;
    der.get(start..start + length)
}

fn asn1_length(der: &[u8]) -> Option<(usize, usize)> {
    let first = *der.first()?;
    if first < 0x80 {
        Some((1, first as usize))
    } else {
        let num_bytes = (first & 0x7f) as usize;
        if num_bytes > 4 || num_bytes == 0 {
            return None;
        }
        let mut length = 0usize;
        for i in 0..num_bytes {
            length = length
                .checked_shl(8)?
                .checked_add(*der.get(1 + i)? as usize)?;
        }
        Some((1 + num_bytes, length))
    }
}

fn parse_asn1_time(tag: u8, bytes: &[u8]) -> Option<std::time::SystemTime> {
    let s = std::str::from_utf8(bytes).ok()?;

    let (year, rest) = if tag == 0x17 {
        let y: i32 = s.get(..2)?.parse().ok()?;
        let year = if y >= 50 { 1900 + y } else { 2000 + y };
        (year, s.get(2..)?)
    } else if tag == 0x18 {
        let year: i32 = s.get(..4)?.parse().ok()?;
        (year, s.get(4..)?)
    } else {
        return None;
    };

    let month: u32 = rest.get(..2)?.parse().ok()?;
    let day: u32 = rest.get(2..4)?.parse().ok()?;
    let hour: u64 = rest.get(4..6)?.parse().ok()?;
    let min: u64 = rest.get(6..8)?.parse().ok()?;
    let sec: u64 = rest.get(8..10)?.parse().ok()?;

    let days_before_month: [u32; 13] = [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let mut days = (year as u32 - 1970) * 365;
    for y in 1970..year {
        if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            days += 1;
        }
    }
    days += days_before_month.get(month as usize)?;
    if month > 2 && year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
        days += 1;
    }
    days += day - 1;

    let secs = u64::from(days) * 86400 + hour * 3600 + min * 60 + sec;
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}
