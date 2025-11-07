use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PumpToken {
    pub mint: String,
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub image_uri: String,
    pub created_timestamp: u64,
    #[serde(rename = "uri")]
    pub metadata_uri: String,
    pub market_cap: f64,
    pub liquidity: f64,
    pub price: f64,
    pub price_change_24h: f64,
    pub is_mint_authority_revoked: bool,
    #[serde(rename = "lp_creation_status")]
    pub lp_status: String,
    #[serde(rename = "creator")]
    pub creator_address: String,
}

#[derive(Debug, Clone)]
pub struct PumpFunScanner {
    client: reqwest::Client,
}

impl PumpFunScanner {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .gzip(true)
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::USER_AGENT,
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".parse().unwrap()
                );
                headers.insert(
                    reqwest::header::ACCEPT,
                    "application/json".parse().unwrap()
                );
                headers.insert(
                    reqwest::header::ORIGIN,
                    "https://pump.fun".parse().unwrap()
                );
                headers.insert(
                    reqwest::header::REFERER,
                    "https://pump.fun".parse().unwrap()
                );
                headers
            })
            .build()
            .expect("Failed to build HTTP client");
        
        Self { client }
    }

    pub async fn get_eligible_tokens(&self) -> Result<Vec<PumpToken>> {
        // Используем beta-эндпоинт — он более стабилен
        let url = "https://frontend-api.pump.fun/coins?limit=50&offset=0&sort=created_timestamp&order=DESC";
        
        log::debug!("Запрос к Pump.fun: {}", url);
        let res = self.client.get(url).send().await?;
        
        let status = res.status();
        let text = res.text().await?;
        
        if !status.is_success() {
            log::error!("Pump.fun вернул {}: {}", status, text);
            anyhow::bail!("HTTP {}: {}", status, text);
        }

        let tokens: Vec<PumpToken> = serde_json::from_str(&text)?;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let filtered: Vec<PumpToken> = tokens
            .into_iter()
            // Только новые (< 15 минут)
            .filter(|t| now.saturating_sub(t.created_timestamp) < 900)
            // Только с отозванным mint
            .filter(|t| t.is_mint_authority_revoked)
            // Только с LP ≥ 5 SOL (реалистичный минимум)
            .filter(|t| t.liquidity >= 5.0)
            // Только активные статусы
            .filter(|t| t.lp_status == "initialized" || t.lp_status == "pending")
            // Рост за 24ч > 20% (фильтр мёртвых)
            .filter(|t| t.price_change_24h > 20.0)
            .collect();

        log::info!("Найдено {} подходящих токенов", filtered.len());
        Ok(filtered)
    }

    pub async fn monitor_eligible_tokens<F>(&self, mut callback: F) -> !
    where
        F: FnMut(Vec<PumpToken>) + Send + 'static,
    {
        loop {
            match self.get_eligible_tokens().await {
                Ok(tokens) if !tokens.is_empty() => {
                    callback(tokens);
                }
                Err(e) => {
                    log::warn!("Ошибка сканирования Pump.fun: {}", e);
                }
                _ => {}
            }
            time::sleep(Duration::from_millis(200)).await;
        }
    }
}