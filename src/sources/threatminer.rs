use crate::error::{Result, SubError};
use crate::{DataSource, IntoSubdomain};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tracing::{info, trace, warn};

#[derive(Deserialize)]
struct ThreatminerResult {
    results: Vec<String>,
}

impl IntoSubdomain for ThreatminerResult {
    //todo: does it have to be HashSet<String> or can we change to HashSet<&str>
    fn subdomains(&self) -> Vec<String> {
        self.results.iter().map(|s| s.to_owned()).collect()
    }
}

#[derive(Default, Clone)]
pub struct ThreatMiner {
    client: Client,
}

impl ThreatMiner {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn build_url(&self, host: &str) -> String {
        format!(
            "https://api.threatminer.org/v2/domain.php?q={}&api=True&rt=5",
            host
        )
    }
}

#[async_trait]
impl DataSource for ThreatMiner {
    async fn run(&self, host: Arc<String>, mut tx: Sender<Vec<String>>) -> Result<()> {
        trace!("fetching data from threatminer for: {}", &host);
        let uri = self.build_url(&host);
        let resp: Option<ThreatminerResult> = self.client.get(&uri).send().await?.json().await?;

        if let Some(data) = resp {
            let subdomains = data.subdomains();
            if !subdomains.is_empty() {
                info!("Discovered {} results for: {}", &subdomains.len(), &host);
                let _ = tx.send(subdomains).await;
                return Ok(());
            }
        }

        warn!("no results found for {} from ThreatMiner", &host);
        Err(SubError::SourceError("ThreatMiner".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matches::matches;
    use tokio::sync::mpsc::channel;

    #[test]
    fn url_builder() {
        let correct_uri = "https://api.threatminer.org/v2/domain.php?q=hackerone.com&api=True&rt=5";
        assert_eq!(
            correct_uri,
            ThreatMiner::default().build_url("hackerone.com")
        );
    }

    // Checks to see if the run function returns subdomains
    #[tokio::test]
    async fn returns_results() {
        let (tx, mut rx) = channel(1);
        let host = Arc::new("hackerone.com".to_owned());
        let _ = ThreatMiner::default().run(host, tx).await;
        let mut results = Vec::new();
        for r in rx.recv().await {
            results.extend(r)
        }
        assert!(!results.is_empty());
    }

    //Some("WaybackMachine couldn't find results for: anVubmxpa2VzdGVh.com")
    #[tokio::test]
    async fn handle_no_results() {
        let (tx, _rx) = channel(1);
        let host = Arc::new("anVubmxpa2VzdGVh.com".to_string());
        assert!(matches!(
            ThreatMiner::default().run(host, tx).await.err().unwrap(),
            SubError::SourceError(_)
        ));
    }
}
