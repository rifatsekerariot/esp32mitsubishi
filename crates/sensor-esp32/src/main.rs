//! Multi-room ESP32 Environment & Presence Sensor Node

use serde::{Deserialize, Serialize};
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomTelemetry {
    pub room_id: String,
    pub room_name: String,
    pub temperature: f32,
    pub humidity: f32,
    pub presence_detected: bool,
    pub motion_energy: u8,
    pub battery_level: Option<u8>,
}

#[cfg(not(target_os = "espidf"))]
fn main() {
    println!("--- ESP32 Room Sensor Node (Desktop / Sim Mode) ---");
    let sample = RoomTelemetry {
        room_id: "living_room".to_string(),
        room_name: "Salon".to_string(),
        temperature: 23.4,
        humidity: 48.2,
        presence_detected: true,
        motion_energy: 75,
        battery_level: Some(100),
    };
    println!("Telemetry payload: {}", serde_json::to_string_pretty(&sample).unwrap());
}

#[cfg(target_os = "espidf")]
fn main() {
    use esp_idf_svc::log::EspLogger;

    EspLogger::initialize_default();
    log::info!("Starting ESP32 Room Sensor Node...");

    let room_id = "living_room";
    let room_name = "Salon";

    loop {
        // Read from BME280 / SHT31 sensor via I2C
        // and LD2410 Radar presence sensor via GPIO / UART
        let telemetry = RoomTelemetry {
            room_id: room_id.to_string(),
            room_name: room_name.to_string(),
            temperature: 22.8,
            humidity: 49.5,
            presence_detected: true,
            motion_energy: 80,
            battery_level: None,
        };

        let json = serde_json::to_string(&telemetry).unwrap();
        log::info!("Publishing telemetry: {}", json);

        // Publish to MQTT topic: home/sensors/{room_id}/state
        // ... (handled via EspMqttClient)

        sleep(Duration::from_secs(5));
    }
}
