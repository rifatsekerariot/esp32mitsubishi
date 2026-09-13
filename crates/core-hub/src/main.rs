mod db;
mod orchestrator;
mod state;
mod web;

use anyhow::Result;
use db::Database;
use orchestrator::ClimateOrchestrator;
use state::AppState;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize structured logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "core_hub=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("=== AuraClimate Central Hub (Raspberry Pi Zero 2 W) ===");

    // 2. Initialize embedded SQLite storage
    let db = Database::new("history.db")?;
    info!("SQLite history database initialized.");

    // 3. Shared application state
    let app_state = AppState::new();

    // 4. Spawn the smart climate orchestrator
    let orchestrator = ClimateOrchestrator::new(app_state.clone());
    tokio::spawn(async move {
        orchestrator.run_loop().await;
    });

    // 5. Spawn periodic database telemetry snapshot task (every 60 seconds)
    let db_clone = db.clone();
    let state_clone = app_state.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60)).await;
            let home = state_clone.home.read().await;
            for sensor in &home.sensors {
                let first_hvac = home.hvacs.first();
                let freq = first_hvac.map(|h| h.compressor_frequency).unwrap_or(0);
                let power = first_hvac.map(|h| h.input_power_watts).unwrap_or(0.0);
                let _ = db_clone.insert_point(
                    &sensor.id,
                    sensor.temperature,
                    sensor.humidity,
                    freq,
                    power,
                );
            }
        }
    });

    // 6. Start Axum HTTP & WebSocket Server
    let router = web::create_router(app_state);
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Tablet Web Dashboard & REST/WS API listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
