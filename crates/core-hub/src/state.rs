//! Multi-HVAC & Multi-Sensor dynamic state management for Core Hub.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HvacUnit {
    pub id: String,
    pub name: String,
    pub model: String,
    pub assigned_sensor_id: Option<String>,
    pub power: String,
    pub mode: String,
    pub target_temperature: f32,
    pub room_temperature: f32,
    pub outside_temperature: Option<f32>,
    pub fan_speed: String,
    pub vane: String,
    pub operating: bool,
    pub compressor_frequency: u8,
    pub input_power_watts: f32,
    pub energy_kwh: f32,
    pub is_remote_temp_active: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorNode {
    pub id: String,
    pub name: String,
    pub room_zone: String,
    pub temperature: f32,
    pub humidity: f32,
    pub presence: bool,
    pub motion_energy: u8,
    pub battery: Option<u8>,
    pub rssi: i32,
    pub last_seen_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeState {
    pub selected_hvac_id: String,
    pub global_mode: String, // "auto_comfort", "sleep", "eco", "away", "manual"
    pub hvacs: Vec<HvacUnit>,
    pub sensors: Vec<SensorNode>,
}

impl Default for HomeState {
    fn default() -> Self {
        Self {
            selected_hvac_id: "ac_living_room".to_string(),
            global_mode: "auto_comfort".to_string(),
            hvacs: vec![
                HvacUnit {
                    id: "ac_living_room".to_string(),
                    name: "Salon Kliması".to_string(),
                    model: "Mitsubishi MSZ-AP35VGK".to_string(),
                    assigned_sensor_id: Some("sensor_living_room".to_string()),
                    power: "On".to_string(),
                    mode: "Heat".to_string(),
                    target_temperature: 22.0,
                    room_temperature: 23.1,
                    outside_temperature: Some(15.4),
                    fan_speed: "Auto".to_string(),
                    vane: "Auto".to_string(),
                    operating: true,
                    compressor_frequency: 32,
                    input_power_watts: 410.0,
                    energy_kwh: 3.1,
                    is_remote_temp_active: true,
                    connected: true,
                },
                HvacUnit {
                    id: "ac_bedroom".to_string(),
                    name: "Yatak Odası Kliması".to_string(),
                    model: "Mitsubishi MSZ-AP25VGK".to_string(),
                    assigned_sensor_id: Some("sensor_bedroom".to_string()),
                    power: "Off".to_string(),
                    mode: "Heat".to_string(),
                    target_temperature: 21.0,
                    room_temperature: 20.8,
                    outside_temperature: Some(15.4),
                    fan_speed: "Quiet".to_string(),
                    vane: "Center".to_string(),
                    operating: false,
                    compressor_frequency: 0,
                    input_power_watts: 0.0,
                    energy_kwh: 1.2,
                    is_remote_temp_active: true,
                    connected: true,
                },
            ],
            sensors: vec![
                SensorNode {
                    id: "sensor_living_room".to_string(),
                    name: "Salon Sensörü".to_string(),
                    room_zone: "Salon".to_string(),
                    temperature: 23.1,
                    humidity: 47.0,
                    presence: true,
                    motion_energy: 78,
                    battery: None, // AC powered
                    rssi: -54,
                    last_seen_secs: 2,
                },
                SensorNode {
                    id: "sensor_bedroom".to_string(),
                    name: "Yatak Odası Sensörü".to_string(),
                    room_zone: "Yatak Odası".to_string(),
                    temperature: 20.8,
                    humidity: 51.5,
                    presence: false,
                    motion_energy: 0,
                    battery: Some(92),
                    rssi: -62,
                    last_seen_secs: 4,
                },
                SensorNode {
                    id: "sensor_office".to_string(),
                    name: "Çalışma Odası Sensörü".to_string(),
                    room_zone: "Çalışma Odası".to_string(),
                    temperature: 22.4,
                    humidity: 45.8,
                    presence: true,
                    motion_energy: 65,
                    battery: Some(88),
                    rssi: -58,
                    last_seen_secs: 1,
                },
                SensorNode {
                    id: "sensor_kids".to_string(),
                    name: "Çocuk Odası Sensörü".to_string(),
                    room_zone: "Çocuk Odası".to_string(),
                    temperature: 22.8,
                    humidity: 49.0,
                    presence: false,
                    motion_energy: 0,
                    battery: Some(95),
                    rssi: -67,
                    last_seen_secs: 6,
                },
            ],
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub home: Arc<RwLock<HomeState>>,
    pub ws_broadcast: broadcast::Sender<String>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(200);
        Self {
            home: Arc::new(RwLock::new(HomeState::default())),
            ws_broadcast: tx,
        }
    }

    /// Broadcast the current state to all connected tablet clients
    pub async fn notify_subscribers(&self) {
        let state = self.home.read().await;
        let payload = serde_json::json!({
            "type": "state_update",
            "payload": *state
        });
        if let Ok(json) = serde_json::to_string(&payload) {
            let _ = self.ws_broadcast.send(json);
        }
    }
}
