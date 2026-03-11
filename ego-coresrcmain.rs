//! Ghost Autonomous Memory Persistence Engine - Core Daemon
//!
//! This is the main entry point for the Ghost daemon that orchestrates
//! state harvesting, encryption, and persistence according to the
//! immutable log architecture.

use ego_core::{GhostConfig, GhostError, GhostResult};
use log::{info, error, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tokio::time::{sleep, Duration};

mod config;
mod orchestrator;
mod plugins;

use config::load_config;
use orchestrator::Orchestrator;

#[tokio::main]
async fn main() -> GhostResult<()> {
    // Initialize logging
    env_logger::init();
    info!("🚀 Ghost Autonomous Memory Persistence Engine v0.1.0 starting...");
    
    // Load configuration
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config/ghost.toml".to_string());
    
    let config = match load_config(&config_path).await {
        Ok(cfg) => {
            info!("✅ Configuration loaded from {}", config_path);
            cfg
        }
        Err(e) => {
            error!("❌ Failed to load configuration: {}", e);
            warn!("Using default configuration");
            GhostConfig::default()
        }
    };
    
    // Initialize orchestrator
    let orchestrator = match Orchestrator::new(config).await {
        Ok(orc) => {
            info!("✅ Orchestrator initialized successfully");
            orc
        }
        Err(e) => {
            error!("❌ Failed to initialize orchestrator: {}", e);
            return Err(e);
        }
    };
    
    // Start the main event loop
    info!("🔄 Starting main event loop");
    run_event_loop(orchestrator).await?;
    
    info!("👋 Ghost daemon shutting down gracefully");
    Ok(())
}

/// Main event loop for the Ghost daemon
async fn run_event_loop(orchestrator: Arc<Orchestrator>) -> GhostResult<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(
        orchestrator.config.harvest_interval_seconds,
    ));
    
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = orchestrator.run_harvest_cycle().await {
                    error!("Harvest cycle failed: {}", e);
                    // Continue despite errors - system must be resilient
                }
            }
            _ = signal::ctrl_c() => {
                info!("Received shutdown signal");
                break;
            }
        }
    }
    
    Ok(())
}