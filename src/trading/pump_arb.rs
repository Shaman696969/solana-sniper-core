use crate::trading::risk::RiskMonitor;
use std::sync::Arc;

async fn start_risk_monitoring(&self, token: &PumpToken, stake_sol: f64) {
    let monitor = Arc::new(RiskMonitor::new(
        self.client.clone(),
        self.wallet.clone(),
        token,
        stake_sol,
    ));
    monitor.start_monitoring().await;
}