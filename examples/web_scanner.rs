use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use solana_sniper_core::scanner::PumpFunScanner;

#[derive(Clone)]
struct AppState {
    scanner: Arc<Mutex<PumpFunScanner>>,
}

#[derive(Deserialize)]
struct WebhookPayload {
    mint: String,
}

#[derive(Serialize)]
struct ApiResponse {
    status: String,
    message: String,
    tokens: Vec<String>,
}

async fn health() -> &'static str {
    "OK"
}

async fn scan_tokens(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse>, (StatusCode, String)> {
    let scanner = state.scanner.lock().await;
    match scanner.get_eligible_tokens().await {
        Ok(tokens) => {
            let mints: Vec<String> = tokens.into_iter().map(|t| t.mint).collect();
            Ok(Json(ApiResponse {
                status: "success".to_string(),
                message: format!("Found {} tokens", mints.len()),
                tokens: mints,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Scan failed: {}", e),
        )),
    }
}

async fn webhook_handler(
    State(_state): State<AppState>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    println!("🔥 Webhook received: {}", payload.mint);
    // Здесь будет логика входа в сделку
    StatusCode::OK
}

#[tokio::main]
async fn main() {
    env_logger::init();
    log::info!("🚀 Starting Pump.fun Scanner on Railway...");

    let scanner = PumpFunScanner::new();
    let app_state = AppState {
        scanner: Arc::new(Mutex::new(scanner)),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/scan", get(scan_tokens))
        .route("/webhook", post(webhook_handler))
        .with_state(app_state);

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8000".to_string())
        .parse()
        .unwrap();

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    log::info!("Listening on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}