//! Smart Multi-Zone Climate Orchestrator.
//!
//! Iterates over all registered HVAC units and pairs them dynamically
//! with assigned or presence-active sensor nodes.

use crate::state::AppState;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

pub struct ClimateOrchestrator {
    state: AppState,
}

impl ClimateOrchestrator {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn run_loop(self) {
        info!("Starting Multi-Zone Climate Orchestration engine...");
        loop {
            sleep(Duration::from_secs(8)).await;
            self.tick().await;
        }
    }

    async fn tick(&self) {
        let mut home = self.state.home.write().await;
        let sensors = home.sensors.clone();
        let is_auto_comfort = home.global_mode == "auto_comfort";

        for hvac in &mut home.hvacs {
            // 1. Determine active sensor for this HVAC unit
            let bound_sensor = if let Some(assigned_id) = &hvac.assigned_sensor_id {
                sensors.iter().find(|s| &s.id == assigned_id).cloned()
            } else {
                // If unassigned, dynamically bind to any room with presence detected
                sensors.iter().find(|s| s.presence).cloned()
            };

            if let Some(sensor) = bound_sensor {
                // Inject the external room sensor temperature
                hvac.room_temperature = sensor.temperature;
                hvac.is_remote_temp_active = true;

                // Smart auto-comfort rule
                if is_auto_comfort && hvac.power == "On" {
                    let delta = (sensor.temperature - hvac.target_temperature).abs();
                    if delta > 1.5 && hvac.fan_speed != "Speed3" && hvac.fan_speed != "Auto" {
                        hvac.fan_speed = "Speed3".to_string();
                    } else if delta < 0.4 && hvac.fan_speed != "Quiet" && hvac.fan_speed != "Auto" {
                        hvac.fan_speed = "Quiet".to_string();
                    }
                }
            }
        }

        drop(home);
        self.state.notify_subscribers().await;
    }
}
