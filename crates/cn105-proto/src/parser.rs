//! Streaming UART frame parser and packet decoder for Mitsubishi CN105 protocol.

use crate::checksum::calculate_checksum;
use crate::temperature::decode_temperature;
use crate::types::*;
use thiserror::Error;

pub const MAX_DATA_BYTES: usize = 64;

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("Checksum mismatch: expected 0x{expected:02x}, got 0x{actual:02x}")]
    ChecksumMismatch { expected: u8, actual: u8 },
    #[error("Packet length too short ({len} bytes)")]
    PacketTooShort { len: usize },
    #[error("Unknown or unsupported command 0x{0:02x}")]
    UnknownCommand(u8),
    #[error("Buffer overflow")]
    BufferOverflow,
}

/// Incoming decoded CN105 event
#[derive(Debug, Clone, PartialEq)]
pub enum Cn105Event {
    Connected { installer: bool },
    UpdateSuccess,
    Settings(HeatpumpSettings),
    RoomTemperature {
        room_temp: f32,
        outside_temp: Option<f32>,
        runtime_hours: Option<f32>,
    },
    Status {
        operating: bool,
        compressor_frequency: u8,
        input_power_w: Option<f32>,
        kwh: Option<f32>,
    },
    StagesAndModes {
        stage: Option<Stage>,
        sub_mode: Option<SubMode>,
    },
    ErrorInfo {
        error_code: u8,
        sub_code: u8,
        is_error: bool,
    },
    Raw {
        command: u8,
        payload: Vec<u8>,
    },
}

/// Streaming byte-by-byte parser that reconstructs frames from UART
#[derive(Debug)]
pub struct FrameParser {
    buffer: [u8; MAX_DATA_BYTES],
    found_start: bool,
    frame_complete: bool,
    bytes_read: usize,
    data_length: isize,
    command: u8,
    checksum_byte: u8,
}

impl Default for FrameParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameParser {
    pub fn new() -> Self {
        Self {
            buffer: [0u8; MAX_DATA_BYTES],
            found_start: false,
            frame_complete: false,
            bytes_read: 0,
            data_length: -1,
            command: 0,
            checksum_byte: 0,
        }
    }

    /// Reset internal state to prepare for the next frame
    pub fn reset(&mut self) {
        self.found_start = false;
        self.frame_complete = false;
        self.bytes_read = 0;
        self.data_length = -1;
        self.command = 0;
        self.checksum_byte = 0;
    }

    /// Feed a single byte into the parser.
    /// Returns `Some(Cn105Event)` when a complete valid frame has been assembled.
    pub fn feed(&mut self, byte: u8) -> Result<Option<Cn105Event>, ParseError> {
        if !self.found_start {
            if byte == 0xFC {
                self.found_start = true;
                self.bytes_read = 0;
                self.buffer[self.bytes_read] = byte;
                self.bytes_read += 1;
            }
            return Ok(None);
        }

        if self.bytes_read >= MAX_DATA_BYTES {
            self.reset();
            return Err(ParseError::BufferOverflow);
        }

        self.buffer[self.bytes_read] = byte;

        // Byte index 4 is the declared data payload length
        if self.bytes_read == 4 {
            self.data_length = byte as isize;
            self.command = self.buffer[1];

            if (self.data_length as usize + 6) > MAX_DATA_BYTES {
                self.reset();
                return Err(ParseError::BufferOverflow);
            }
        }

        // Frame is complete when bytes_read == 5 (header) + data_length + 1 (checksum) - 1 (0-indexed)
        if self.data_length >= 0 && self.bytes_read == (self.data_length as usize + 5) {
            self.checksum_byte = byte;
            self.frame_complete = true;

            let computed_checksum = calculate_checksum(&self.buffer[..self.bytes_read]);
            if computed_checksum != self.checksum_byte {
                let err = ParseError::ChecksumMismatch {
                    expected: computed_checksum,
                    actual: self.checksum_byte,
                };
                self.reset();
                return Err(err);
            }

            let payload = &self.buffer[5..self.bytes_read];
            let event = Self::decode_frame(self.command, payload);
            self.reset();
            return Ok(Some(event));
        } else {
            self.bytes_read += 1;
        }

        Ok(None)
    }

    /// Decode the command and payload bytes into a structured `Cn105Event`
    pub fn decode_frame(command: u8, payload: &[u8]) -> Cn105Event {
        match command {
            0x7A => Cn105Event::Connected { installer: false },
            0x7B => Cn105Event::Connected { installer: true },
            0x61 => Cn105Event::UpdateSuccess,
            0x62 => {
                if payload.is_empty() {
                    return Cn105Event::Raw {
                        command,
                        payload: payload.to_vec(),
                    };
                }
                let sub_type = payload[0];
                match sub_type {
                    0x02 => Self::decode_settings(payload),
                    0x03 => Self::decode_room_temp(payload),
                    0x04 => Self::decode_error_info(payload),
                    0x06 => Self::decode_status(payload),
                    0x09 => Self::decode_stages_and_modes(payload),
                    _ => Cn105Event::Raw {
                        command,
                        payload: payload.to_vec(),
                    },
                }
            }
            _ => Cn105Event::Raw {
                command,
                payload: payload.to_vec(),
            },
        }
    }

    fn decode_settings(payload: &[u8]) -> Cn105Event {
        // payload[0] = 0x02
        // payload[3] = power
        // payload[4] = mode (+ isee flag if > 0x08)
        // payload[5] = temp A
        // payload[6] = fan
        // payload[7] = vane
        // payload[10] = widevane
        // payload[11] = temp B
        // payload[12] = target humidity (on some models)
        let mut settings = HeatpumpSettings {
            connected: true,
            ..Default::default()
        };

        if payload.len() > 3 {
            settings.power = Power::from_byte(payload[3]);
        }
        if payload.len() > 4 {
            let isee = payload[4] > 0x08;
            settings.isee = isee;
            let mode_byte = if isee { payload[4] - 0x08 } else { payload[4] };
            settings.mode = Mode::from_byte(mode_byte);
        }
        if payload.len() > 11 {
            let enc_a = payload[5];
            let enc_b = payload[11];
            settings.target_temperature = Some(decode_temperature(enc_a, enc_b, 10));
        }
        if payload.len() > 6 {
            settings.fan_speed = FanSpeed::from_byte(payload[6]);
        }
        if payload.len() > 7 {
            settings.vane = VanePosition::from_byte(payload[7]);
        }
        if payload.len() > 10 {
            settings.wide_vane = WideVane::from_byte(payload[10]);
        }
        if payload.len() > 12 && payload[12] > 0 && payload[12] <= 100 {
            settings.target_humidity = Some(payload[12]);
        }

        Cn105Event::Settings(settings)
    }

    fn decode_room_temp(payload: &[u8]) -> Cn105Event {
        // payload[3] = enc_a, payload[6] = enc_b
        // payload[5] = outside air temp (if > 1: (val - 128) / 2.0)
        // payload[11..13] = runtime hours ((val << 16) | (val << 8) | val) / 60
        let room_temp = if payload.len() > 6 && payload[6] != 0x00 {
            (payload[6] as f32 - 128.0) / 2.0
        } else if payload.len() > 3 {
            payload[3] as f32 + 10.0
        } else {
            0.0
        };

        let outside_temp = if payload.len() > 5 && payload[5] > 1 {
            Some((payload[5] as f32 - 128.0) / 2.0)
        } else {
            None
        };

        let runtime_hours = if payload.len() > 13 {
            let mins = ((payload[11] as u32) << 16)
                | ((payload[12] as u32) << 8)
                | (payload[13] as u32);
            Some(mins as f32 / 60.0)
        } else {
            None
        };

        Cn105Event::RoomTemperature {
            room_temp,
            outside_temp,
            runtime_hours,
        }
    }

    fn decode_status(payload: &[u8]) -> Cn105Event {
        // payload[3] = compressor frequency
        // payload[4] = operating status (0 = standby, 1 = running)
        // payload[5..7] = input power in Watts (16-bit)
        // payload[7..9] = energy in kWh (value / 10)
        let operating = payload.get(4).copied().unwrap_or(0) == 1;
        let compressor_frequency = if operating {
            payload.get(3).copied().unwrap_or(0)
        } else {
            0
        };

        let input_power_w = if payload.len() > 6 {
            let raw = ((payload[5] as u16) << 8) | (payload[6] as u16);
            Some(raw as f32)
        } else {
            None
        };

        let kwh = if payload.len() > 8 {
            let raw = ((payload[7] as u16) << 8) | (payload[8] as u16);
            Some(raw as f32 / 10.0)
        } else {
            None
        };

        Cn105Event::Status {
            operating,
            compressor_frequency,
            input_power_w,
            kwh,
        }
    }

    fn decode_stages_and_modes(payload: &[u8]) -> Cn105Event {
        let sub_mode = payload.get(3).copied().and_then(SubMode::from_byte);
        let stage = payload.get(4).copied().and_then(Stage::from_byte);
        Cn105Event::StagesAndModes { stage, sub_mode }
    }

    fn decode_error_info(payload: &[u8]) -> Cn105Event {
        let error_raw = payload.get(4).copied().unwrap_or(0);
        let sub_code = payload.get(5).copied().unwrap_or(0);
        let error_code = error_raw & 0x7F;
        let is_error = error_code != 0 || sub_code != 0;
        Cn105Event::ErrorInfo {
            error_code,
            sub_code,
            is_error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_parser_status_packet() {
        // Real packet from heatpump:
        // FC 62 01 30 10 06 00 00 1A 01 00 00 00 00 00 00 00 00 00 00 00 3C
        let raw_packet = [
            0xFC, 0x62, 0x01, 0x30, 0x10, 0x06, 0x00, 0x00, 0x1A, 0x01, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3C,
        ];

        let mut parser = FrameParser::new();

        // Feed garbage bytes before packet
        assert_eq!(parser.feed(0x00).unwrap(), None);
        assert_eq!(parser.feed(0xFF).unwrap(), None);
        assert_eq!(parser.feed(0xAA).unwrap(), None);

        // Feed packet bytes
        let mut last_event = None;
        for &b in &raw_packet {
            if let Some(event) = parser.feed(b).unwrap() {
                last_event = Some(event);
            }
        }

        assert_eq!(
            last_event,
            Some(Cn105Event::Status {
                operating: true,
                compressor_frequency: 0x1A, // 26 Hz
                input_power_w: Some(0.0),
                kwh: Some(0.0),
            })
        );
    }

    #[test]
    fn test_stream_parser_update_success() {
        // 0x61 ACK packet: FC 61 01 30 00 chk
        // len = 0, total bytes = 5 + 0 + 1 = 6
        let mut packet = vec![0xFC, 0x61, 0x01, 0x30, 0x00];
        let chk = calculate_checksum(&packet);
        packet.push(chk);

        let mut parser = FrameParser::new();
        let mut last_event = None;
        for &b in &packet {
            if let Some(ev) = parser.feed(b).unwrap() {
                last_event = Some(ev);
            }
        }

        assert_eq!(last_event, Some(Cn105Event::UpdateSuccess));
    }
}

