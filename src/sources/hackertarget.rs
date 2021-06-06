use crate::error::{Result, SubError};
use crate::{DataSource, IntoSubdomain};
use async_trait::async_trait;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tracing::{info, trace, warn};

const API_ERROR: &str = "error check your search parameter";

struct HTResult {
    items: String,
}

impl HTResult {
    fn new(items: String) -> Self {
        HTResult { items }
    }
}

impl IntoSubdomain for HTResult {
    fn subdomains(&self) -> Vec<String> {
        self.items
            .lines()
            .map(|s| s.split(',').collect::<Vec<&str>>()[0].to_owned())
            .collect()
    }
}

#[derive(Default, Clone)]
pub struct HackerTarget {
    client: Client,
}

impl HackerTarget {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn build_url(&self, host: &str) -> String {
        format!("https://api.hackertarget.com/hostsearch/?q={}", host)
    }
}

#[async_trait]
impl DataSource for HackerTarget {
    async fn run(&self, host: Arc<String>, mut tx: Sender<Vec<String>>) -> Result<()> {
        trace!("fetching data from hackertarget for: {}", &host);
        let uri = self.build_url(&host);
        let resp: String = self.client.get(&uri).send().await?.text().await?;

        if resp != API_ERROR {
            let subdomains = HTResult::new(resp).subdomains();
            info!("Discovered {} results for: {}", &subdomains.len(), &host);
            let _ = tx.send(subdomains).await;
            return Ok(());
        }

        warn!("no results found for {} from HackerTarget", &host);
        Err(SubError::SourceError("HackerTarget".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matches::matches;
    use tokio::sync::mpsc::channel;

    #[tokio::test]
    async fn returns_results() {
        let (tx, mut rx) = channel(1);
        let host = Arc::new("hackerone.com".to_owned());
        let _ = HackerTarget::default().run(host, tx).await;
        let mut results = Vec::new();
        for r in rx.recv().await {
            results.extend(r)
        }
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn handle_no_results() {
        let (tx, _rx) = channel(1);
        let host = Arc::new("anVubmxpa2VzdGVh.com".to_string());
        assert!(matches!(
            HackerTarget::default().run(host, tx).await.err().unwrap(),
            SubError::SourceError(_)
        ));
    }
}
