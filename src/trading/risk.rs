use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time;

use crate::scanner::PumpToken;

#[derive(Debug, Clone)]
pub struct RiskMonitor {
    client: RpcClient,
    wallet: Keypair,
    token_mint: Pubkey,
    entry_price: f64,
    stake_sol: f64,
    moon_allocation: f64, // 20% от позиции
    peak_price: f64,
    start_time: Instant,
}

impl RiskMonitor {
    pub fn new(
        client: RpcClient,
        wallet: Keypair,
        token: &PumpToken,
        stake_sol: f64,
    ) -> Self {
        let mint = Pubkey::from_str(&token.mint).unwrap_or_default();
        Self {
            client,
            wallet,
            token_mint: mint,
            entry_price: token.price,
            stake_sol,
            moon_allocation: stake_sol * 0.2, // 20% — "На Луну"
            peak_price: token.price,
            start_time: Instant::now(),
        }
    }

    /// Запуск фонового мониторинга
    pub async fn start_monitoring(self: Arc<Self>) {
        let mut interval = time::interval(Duration::from_millis(500));
        let client = self.client.clone();

        tokio::spawn(async move {
            loop {
                interval.tick().await;
                if let Err(e) = self.check_risk_conditions(&client).await {
                    log::error!("Ошибка мониторинга рисков: {}", e);
                    break;
                }
            }
        });
    }

    /// Проверка всех условий выхода
    async fn check_risk_conditions(&self, client: &RpcClient) -> Result<()> {
        // 1. Получаем текущую цену и данные пула
        let (current_price, quote_reserve) = self.get_price_and_liquidity(client).await?;

        // Обновляем пик
        if current_price > self.peak_price {
            self.peak_price = current_price;
        }

        // 2. Трёхуровневый стоп-лосс
        self.check_rug_pull(quote_reserve).await?;
        self.check_panic_sell(current_price).await?;
        self.check_time_decay().await?;

        // 3. Moon Mode: условия выхода
        self.check_moon_exit(current_price, quote_reserve).await?;

        Ok(())
    }

    async fn get_price_and_liquidity(&self, client: &RpcClient) -> Result<(f64, u64)> {
        // В реальном коде: запрос к Jupiter или Raydium pool
        // Для MVP: имитация через API или кэш
        Ok((self.entry_price * 1.05, 10_000_000_000)) // +5%, 10 SOL в пуле
    }

    /// Уровень 1: Rug-pull — падение резерва на ≥40%
    async fn check_rug_pull(&self, current_reserve: u64) -> Result<()> {
        let initial_reserve = 10_000_000_000; // имитация; в реале — из пула на входе
        let drop_ratio = 1.0 - (current_reserve as f64 / initial_reserve as f64);
        
        if drop_ratio >= 0.4 {
            log::error!("🚨 RUG-PULL DETECTED! Резерв упал на {:.1}%", drop_ratio * 100.0);
            self.emergency_sell(1.0).await?; // продаём 100%
        }
        Ok(())
    }

    /// Уровень 2: Panic-sell — цена ↓60% за 30 сек или серия мелких свечей
    async fn check_panic_sell(&self, current_price: f64) -> Result<()> {
        let drawdown = (self.entry_price - current_price) / self.entry_price;
        let elapsed = self.start_time.elapsed().as_secs();

        // Если цена упала на 60% — экстренная продажа ВСЕГО
        if drawdown >= 0.6 {
            log::error!("🔥 PANIC SELL! Цена упала на {:.1}%", drawdown * 100.0);
            self.emergency_sell(1.0).await?;
        }
        // Если нет роста 90 сек — продаём 50%
        else if elapsed > 90 && current_price < self.entry_price * 1.1 {
            log::warn!("⏳ Time-out: нет роста 90 сек → частичная продажа");
            self.emergency_sell(0.5).await?;
        }
        Ok(())
    }

    /// Уровень 3: Trailing stop — 30% от максимума
    async fn check_time_decay(&self) -> Result<()> {
        let drawdown_from_peak = (self.peak_price - self.entry_price * 1.0) / self.peak_price;
        if drawdown_from_peak >= 0.3 && self.peak_price > self.entry_price {
            log::info!("📉 Trailing stop: падение на 30% от пика → продажа остатка");
            self.emergency_sell(1.0).await?; // закрываем всё
        }
        Ok(())
    }

    /// Moon Mode: умный выход для 20% позиции
    async fn check_moon_exit(&self, current_price: f64, _quote_reserve: u64) -> Result<()> {
        let moon_multiplier = current_price / self.entry_price;

        // Условие 1: +50x И объём > 1M SOL (в реале — через DexScreener API)
        if moon_multiplier >= 50.0 {
            log::info!("🌕 MOON MODE: +{:.0}x → фиксируем лунную долю!", moon_multiplier);
            self.sell_moon_position().await?;
            return Ok(());
        }

        // Условие 2: попадание в топ-3 DexScreener (имитация)
        // if is_in_dexscreener_top3(&self.token_mint).await {
        //     log::info!("🌕 MOON MODE: в топ-3 DexScreener → фиксируем!");
        //     self.sell_moon_position().await?;
        //     return Ok(());
        // }

        // Условие 3: таймер 24 часа
        if self.start_time.elapsed().as_secs() > 86400 {
            log::info!("🌕 MOON MODE: 24 часа → auto-sell лунной доли");
            self.sell_moon_position().await?;
        }

        Ok(())
    }

    /// Экстренная продажа (часть или всё)
    async fn emergency_sell(&self, fraction: f64) -> Result<()> {
        let amount_to_sell = self.stake_sol * fraction;
        log::info!("📤 Экстренная продажа {} SOL ({}%)", amount_to_sell, fraction * 100.0);
        // Здесь — вызов Jupiter swap SOL ← token
        Ok(())
    }

    /// Продажа "лунной доли"
    async fn sell_moon_position(&self) -> Result<()> {
        log::info!("🌕 Продажа лунной доли: {:.4} SOL", self.moon_allocation);
        self.emergency_sell(self.moon_allocation / self.stake_sol).await
    }
}