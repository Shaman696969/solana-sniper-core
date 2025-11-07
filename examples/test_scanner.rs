use log::{info, warn, LevelFilter};
use solana_sniper_core::scanner::PumpFunScanner;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .init();

    info!("Запуск тестового сканера Pump.fun...");

    let scanner = PumpFunScanner::new();
    
    match scanner.get_eligible_tokens().await {
        Ok(tokens) => {
            info!("Найдено подходящих токенов: {}", tokens.len());
            for t in tokens {
                info!(
                    "  {} ({}) — LP: {:.2} SOL, рост: {:+.1}%, статус: {}",
                    t.symbol,
                    &t.mint[..8],
                    t.liquidity,
                    t.price_change_24h,
                    t.lp_status
                );
            }
        }
        Err(e) => {
            warn!("Не удалось получить токены: {}", e);
        }
    }

    Ok(())
}