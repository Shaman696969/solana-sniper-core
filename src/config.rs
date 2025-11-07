use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub rpc_url: String,
    pub wallets: Vec<String>,
    pub buy_amount_sol: f64,    // % от капитала (10.0 = 10%)
    pub jito_region: String,
    pub dry_run: bool,
}