use crate::error::{Result, SubError};
use crate::{DataSource, IntoSubdomain};
use async_trait::async_trait;
use dotenv::dotenv;
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tracing::{info, trace, warn};

struct Creds {
    key: String,
}

impl Creds {
    pub fn read_creds() -> Result<Self> {
        dotenv().ok();
        match env::var("C99_KEY") {
            Ok(key) => Ok(Self { key }),
            Err(_) => Err(SubError::UnsetKeys(Sub!["C99_KEY".into()])),
        }
    }
}

#[derive(Debug, Deserialize)]
struct C99Result {
    subdomains: Option<Sub<C99Item>>,
}

#[derive(Debug, Deserialize)]
struct C99Item {
    subdomain: String,
}

impl IntoSubdomain for C99Result {
    fn subdomains(&self) -> Sub<String> {
        self.subdomains
            .iter()
            .flatten()
            .map(|s| s.subdomain.to_string())
            .collect()
    }
}

#[derive(Default, Clone)]
pub struct C99 {
    client: Client,
}

impl C99 {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn build_url(&self, host: &str, api_key: &str) -> String {
        format!(
            "https://api.c99.nl/subdomainfinder?key={}&domain={}&json",
            api_key, host
        )
    }
}

#[async_trait]
impl DataSource for C99 {
    async fn run(&self, host: Arc<String>, mut tx: Sender<Sub<String>>) -> Result<()> {
        trace!("fetching data from C99 for: {}", &host);
        let api_key = match Creds::read_creds() {
            Ok(creds) => creds.key,
            Err(e) => return Err(e),
        };

        let uri = self.build_url(&host, &api_key);
        let resp = self.client.get(&uri).send().await?;

        if resp.status().is_success() {
            let resp: C99Result = resp.json().await?;
            let subdomains = resp.subdomains();
            if !subdomains.is_empty() {
                info!("Discovered {} results for {}", &subdomains.len(), &host);
                let _ = tx.send(subdomains).await;
                return Ok(());
            }
        }

        warn!("no results for {} from C99", &host);
        Err(SubError::SourceError("C99".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matches::matches;
    use tokio::sync::mpsc::channel;

    #[ignore]
    #[tokio::test]
    async fn returns_results() {
        let (tx, mut rx) = channel(1);
        let host = Arc::new("hackerone.com".to_owned());
        let _ = C99::default().run(host, tx).await;
        let mut results = Sub::new();
        for r in rx.recv().await {
            results.extend(r)
        }
        assert!(!results.is_empty());
    }

    #[ignore]
    #[tokio::test]
    async fn handle_no_results() {
        let (tx, _rx) = channel(1);
        let host = Arc::new("anVubmxpa2VzdGVh.com".to_string());
        assert!(matches!(
            C99::default().run(host, tx).await.err().unwrap(),
            SubError::SourceError(_)
        ));
    }
}
