//! HVAC Controller logic and state management for Mitsubishi CN105.

use cn105_proto::{
    build_connect_packet, build_info_request_packet, build_remote_temp_packet,
    build_settings_packet, Cn105Event, FrameParser, HeatpumpSettings, HeatpumpStatus,
    WantedSettings,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const QUERY_INTERVAL: Duration = Duration::from_secs(10);
pub const REMOTE_TEMP_KEEPALIVE: Duration = Duration::from_secs(20);
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Combined system state published to MQTT
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HvacPublishedState {
    pub connected: bool,
    pub power: Option<String>,
    pub mode: Option<String>,
    pub target_temperature: Option<f32>,
    pub room_temperature: f32,
    pub outside_temperature: Option<f32>,
    pub fan_speed: Option<String>,
    pub vane: Option<String>,
    pub wide_vane: Option<String>,
    pub operating: bool,
    pub compressor_frequency: u8,
    pub input_power_watts: Option<f32>,
    pub energy_kwh: Option<f32>,
    pub runtime_hours: Option<f32>,
    pub is_remote_temp_active: bool,
    pub last_updated_secs_ago: u64,
}

pub struct HvacController {
    pub settings: HeatpumpSettings,
    pub status: HeatpumpStatus,
    pub wanted_settings: Option<WantedSettings>,
    pub remote_temperature: Option<f32>,
    pub is_connected: bool,
    pub parser: FrameParser,

    last_query: Instant,
    last_remote_temp_send: Instant,
    last_connect_attempt: Instant,
    last_state_change: Instant,
    current_query_index: u8,
}

impl Default for HvacController {
    fn default() -> Self {
        Self::new()
    }
}

impl HvacController {
    pub fn new() -> Self {
        let now = Instant::now() - Duration::from_secs(60);
        Self {
            settings: HeatpumpSettings::default(),
            status: HeatpumpStatus::default(),
            wanted_settings: None,
            remote_temperature: None,
            is_connected: false,
            parser: FrameParser::new(),
            last_query: now,
            last_remote_temp_send: now,
            last_connect_attempt: now,
            last_state_change: Instant::now(),
            current_query_index: 0,
        }
    }

    /// Process an incoming byte from UART
    pub fn on_uart_byte(&mut self, byte: u8) -> Option<Cn105Event> {
        match self.parser.feed(byte) {
            Ok(Some(event)) => {
                self.handle_event(&event);
                Some(event)
            }
            Ok(None) => None,
            Err(e) => {
                log::warn!("UART parse error: {:?}", e);
                None
            }
        }
    }

    /// Internal handler to update settings/status structs on incoming events
    fn handle_event(&mut self, event: &Cn105Event) {
        match event {
            Cn105Event::Connected { installer } => {
                self.is_connected = true;
                log::info!("Heatpump connected (installer={})", installer);
            }
            Cn105Event::UpdateSuccess => {
                log::info!("Heatpump acknowledged settings update");
            }
            Cn105Event::Settings(s) => {
                self.settings = s.clone();
                self.is_connected = true;
                self.last_state_change = Instant::now();
            }
            Cn105Event::RoomTemperature {
                room_temp,
                outside_temp,
                runtime_hours,
            } => {
                self.status.room_temperature = *room_temp;
                if outside_temp.is_some() {
                    self.status.outside_temperature = *outside_temp;
                }
                if runtime_hours.is_some() {
                    self.status.runtime_hours = *runtime_hours;
                }
                self.last_state_change = Instant::now();
            }
            Cn105Event::Status {
                operating,
                compressor_frequency,
                input_power_w,
                kwh,
            } => {
                self.status.operating = *operating;
                self.status.compressor_frequency = *compressor_frequency;
                if input_power_w.is_some() {
                    self.status.input_power_watts = *input_power_w;
                }
                if kwh.is_some() {
                    self.status.energy_kwh = *kwh;
                }
                self.last_state_change = Instant::now();
            }
            Cn105Event::StagesAndModes { stage, sub_mode } => {
                self.status.stage = *stage;
                self.status.sub_mode = *sub_mode;
            }
            Cn105Event::ErrorInfo {
                error_code,
                sub_code,
                is_error,
            } => {
                if *is_error {
                    log::warn!("Heatpump reported error: 0x{:02x}:{:02x}", error_code, sub_code);
                }
            }
            Cn105Event::Raw { command, payload } => {
                log::debug!("Raw frame cmd=0x{:02x}, len={}", command, payload.len());
            }
        }
    }

    /// Schedule outgoing packets (connection handshake, queries, wanted settings, or remote temp)
    pub fn get_next_outgoing_packet(&mut self) -> Option<Vec<u8>> {
        let now = Instant::now();

        // 1. If not connected, attempt handshake periodically
        if !self.is_connected {
            if now.duration_since(self.last_connect_attempt) > CONNECT_TIMEOUT {
                self.last_connect_attempt = now;
                log::info!("Sending connect handshake packet...");
                return Some(build_connect_packet(false).to_vec());
            }
            return None;
        }

        // 2. High priority: Wanted settings command requested by user / MQTT
        if let Some(wanted) = self.wanted_settings.take() {
            log::info!("Sending user wanted settings update: {:?}", wanted);
            return Some(build_settings_packet(&wanted).to_vec());
        }

        // 3. Keepalive remote temperature (sent every 20s if set)
        if self.remote_temperature.is_some()
            && now.duration_since(self.last_remote_temp_send) > REMOTE_TEMP_KEEPALIVE
        {
            self.last_remote_temp_send = now;
            log::info!(
                "Sending remote temp keepalive: {:.1}°C",
                self.remote_temperature.unwrap()
            );
            return Some(build_remote_temp_packet(self.remote_temperature).to_vec());
        }

        // 4. Polling query cycle for status / settings / room temp
        if now.duration_since(self.last_query) > (QUERY_INTERVAL / 4) {
            self.last_query = now;
            let codes = [0x02, 0x03, 0x04, 0x06]; // Settings, Temp, Errors, Operating
            let code = codes[self.current_query_index as usize % codes.len()];
            self.current_query_index = (self.current_query_index + 1) % codes.len() as u8;
            return Some(build_info_request_packet(code).to_vec());
        }

        None
    }

    /// Sets wanted settings from user/MQTT
    pub fn queue_wanted_settings(&mut self, wanted: WantedSettings) {
        self.wanted_settings = Some(wanted);
    }

    /// Updates the remote temperature reference
    pub fn set_remote_temperature(&mut self, temp: Option<f32>) {
        self.remote_temperature = temp;
        // Trigger immediate send on next iteration
        self.last_remote_temp_send = Instant::now() - REMOTE_TEMP_KEEPALIVE;
    }

    /// Produces a snapshot for MQTT / Web publishing
    pub fn create_published_state(&self) -> HvacPublishedState {
        HvacPublishedState {
            connected: self.is_connected,
            power: self.settings.power.map(|p| format!("{:?}", p)),
            mode: self.settings.mode.map(|m| format!("{:?}", m)),
            target_temperature: self.settings.target_temperature,
            room_temperature: self.status.room_temperature,
            outside_temperature: self.status.outside_temperature,
            fan_speed: self.settings.fan_speed.map(|f| format!("{:?}", f)),
            vane: self.settings.vane.map(|v| format!("{:?}", v)),
            wide_vane: self.settings.wide_vane.map(|w| format!("{:?}", w)),
            operating: self.status.operating,
            compressor_frequency: self.status.compressor_frequency,
            input_power_watts: self.status.input_power_watts,
            energy_kwh: self.status.energy_kwh,
            runtime_hours: self.status.runtime_hours,
            is_remote_temp_active: self.remote_temperature.is_some(),
            last_updated_secs_ago: self.last_state_change.elapsed().as_secs(),
        }
    }
}
