//! Typed enums and models for Mitsubishi CN105 protocol.

use serde::{Deserialize, Serialize};

/// Operating power state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Power {
    Off = 0x00,
    On = 0x01,
}

impl Power {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Power::Off),
            0x01 => Some(Power::On),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// HVAC operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Heat = 0x01,
    Dry = 0x02,
    Cool = 0x03,
    Fan = 0x07,
    Auto = 0x08,
}

impl Mode {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Mode::Heat),
            0x02 => Some(Mode::Dry),
            0x03 => Some(Mode::Cool),
            0x07 => Some(Mode::Fan),
            0x08 => Some(Mode::Auto),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Fan speed setting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FanSpeed {
    Auto = 0x00,
    Quiet = 0x01,
    Speed1 = 0x02,
    Speed2 = 0x03,
    Speed3 = 0x05,
    Speed4 = 0x06,
}

impl FanSpeed {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(FanSpeed::Auto),
            0x01 => Some(FanSpeed::Quiet),
            0x02 => Some(FanSpeed::Speed1),
            0x03 => Some(FanSpeed::Speed2),
            0x05 => Some(FanSpeed::Speed3),
            0x06 => Some(FanSpeed::Speed4),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Vertical vane (louver) direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VanePosition {
    Auto = 0x00,
    Up = 0x01,
    UpCenter = 0x02,
    Center = 0x03,
    DownCenter = 0x04,
    Down = 0x05,
    Swing = 0x07,
}

impl VanePosition {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(VanePosition::Auto),
            0x01 => Some(VanePosition::Up),
            0x02 => Some(VanePosition::UpCenter),
            0x03 => Some(VanePosition::Center),
            0x04 => Some(VanePosition::DownCenter),
            0x05 => Some(VanePosition::Down),
            0x07 => Some(VanePosition::Swing),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Horizontal wide-vane direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WideVane {
    LeftMax = 0x01,
    Left = 0x02,
    Center = 0x03,
    Right = 0x04,
    RightMax = 0x05,
    Split = 0x08,
    Swing = 0x0C,
    AirflowControl = 0x00,
}

impl WideVane {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b & 0x0F {
            0x01 => Some(WideVane::LeftMax),
            0x02 => Some(WideVane::Left),
            0x03 => Some(WideVane::Center),
            0x04 => Some(WideVane::Right),
            0x05 => Some(WideVane::RightMax),
            0x08 => Some(WideVane::Split),
            0x0C => Some(WideVane::Swing),
            0x00 => Some(WideVane::AirflowControl),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Sub-mode status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubMode {
    Normal = 0x00,
    Warmup = 0x01,
    Defrost = 0x02,
    Preheat = 0x04,
    Standby = 0x08,
    Off = 0x10,
}

impl SubMode {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(SubMode::Normal),
            0x01 => Some(SubMode::Warmup),
            0x02 => Some(SubMode::Defrost),
            0x04 => Some(SubMode::Preheat),
            0x08 => Some(SubMode::Standby),
            0x10 => Some(SubMode::Off),
            _ => None,
        }
    }
}

/// Operating stage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    Idle = 0x00,
    Low = 0x01,
    Gentle = 0x02,
    Medium = 0x03,
    Moderate = 0x04,
    High = 0x05,
    Diffuse = 0x06,
}

impl Stage {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Stage::Idle),
            0x01 => Some(Stage::Low),
            0x02 => Some(Stage::Gentle),
            0x03 => Some(Stage::Medium),
            0x04 => Some(Stage::Moderate),
            0x05 => Some(Stage::High),
            0x06 => Some(Stage::Diffuse),
            _ => None,
        }
    }
}

/// Current active settings of the heatpump
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HeatpumpSettings {
    pub power: Option<Power>,
    pub mode: Option<Mode>,
    pub target_temperature: Option<f32>,
    pub fan_speed: Option<FanSpeed>,
    pub vane: Option<VanePosition>,
    pub wide_vane: Option<WideVane>,
    pub isee: bool,
    pub target_humidity: Option<u8>,
    pub connected: bool,
}

/// Operational status and diagnostic readings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HeatpumpStatus {
    pub operating: bool,
    pub compressor_frequency: u8,
    pub room_temperature: f32,
    pub outside_temperature: Option<f32>,
    pub input_power_watts: Option<f32>,
    pub energy_kwh: Option<f32>,
    pub runtime_hours: Option<f32>,
    pub stage: Option<Stage>,
    pub sub_mode: Option<SubMode>,
}

/// Wanted settings to send as an update command
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WantedSettings {
    pub power: Option<Power>,
    pub mode: Option<Mode>,
    pub temperature: Option<f32>,
    pub fan_speed: Option<FanSpeed>,
    pub vane: Option<VanePosition>,
    pub wide_vane: Option<WideVane>,
}
