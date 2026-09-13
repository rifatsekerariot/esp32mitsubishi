//! CN105 Packet construction utilities.

use crate::checksum::calculate_checksum;
use crate::temperature::{encode_remote_temperature, encode_temperature_b};
use crate::types::*;

pub const PACKET_LEN: usize = 22;
pub const CONNECT_LEN: usize = 8;

pub const CONNECT_PACKET: [u8; CONNECT_LEN] = [0xfc, 0x5a, 0x01, 0x30, 0x02, 0xca, 0x01, 0xa8];
pub const HEADER: [u8; 8] = [0xfc, 0x41, 0x01, 0x30, 0x10, 0x01, 0x00, 0x00];
pub const INFO_HEADER: [u8; 5] = [0xfc, 0x42, 0x01, 0x30, 0x10];

/// Builds a connection handshake packet.
///
/// # Arguments
/// * `installer` - True for installer handshake (0x5B), false for standard (0x5A)
pub fn build_connect_packet(installer: bool) -> [u8; CONNECT_LEN] {
    let mut packet = CONNECT_PACKET;
    packet[1] = if installer { 0x5B } else { 0x5A };
    packet[CONNECT_LEN - 1] = calculate_checksum(&packet[..CONNECT_LEN - 1]);
    packet
}

/// Builds an information request packet (e.g. status, room temp, settings).
///
/// Standard codes:
/// - `0x02`: Settings
/// - `0x03`: Room Temperature
/// - `0x04`: Error Info
/// - `0x06`: Operating Status / Power / kWh
/// - `0x09`: Sub-modes / Stage
pub fn build_info_request_packet(code: u8) -> [u8; PACKET_LEN] {
    let mut packet = [0u8; PACKET_LEN];
    packet[..5].copy_from_slice(&INFO_HEADER);
    packet[5] = code;
    packet[PACKET_LEN - 1] = calculate_checksum(&packet[..PACKET_LEN - 1]);
    packet
}

/// Builds a remote temperature enjection packet (SET 0x07).
///
/// Used to supply an external room temperature reading to the heat pump,
/// bypassing its internal high-wall / ceiling sensor.
pub fn build_remote_temp_packet(temp_celsius: Option<f32>) -> [u8; PACKET_LEN] {
    let mut packet = [0u8; PACKET_LEN];
    packet[..8].copy_from_slice(&HEADER);
    packet[5] = 0x07;

    if let Some(temp) = temp_celsius {
        packet[6] = 0x01;
        let (enc_a, enc_b) = encode_remote_temperature(temp);
        packet[7] = enc_a;
        packet[8] = enc_b;
    } else {
        packet[8] = 0x80;
    }

    packet[PACKET_LEN - 1] = calculate_checksum(&packet[..PACKET_LEN - 1]);
    packet
}

/// Builds a command update packet with the specified wanted settings.
pub fn build_settings_packet(wanted: &WantedSettings) -> [u8; PACKET_LEN] {
    let mut packet = [0u8; PACKET_LEN];
    packet[..8].copy_from_slice(&HEADER);

    let mut control_flags_1 = 0u8;
    let mut control_flags_2 = 0u8;

    if let Some(power) = wanted.power {
        control_flags_1 |= 0x01;
        packet[8] = power.to_byte();
    }

    if let Some(mode) = wanted.mode {
        control_flags_1 |= 0x02;
        packet[9] = mode.to_byte();
    }

    if let Some(temp) = wanted.temperature {
        control_flags_1 |= 0x04;
        // Provide both legacy mapping byte and Encoding B byte for broad model compatibility
        let enc_b = encode_temperature_b(temp);
        packet[19] = enc_b;
        // Legacy index (31 down to 16 mapped to 0..15)
        let temp_int = temp.round() as i32;
        if (16..=31).contains(&temp_int) {
            packet[10] = (31 - temp_int) as u8;
        }
    }

    if let Some(fan) = wanted.fan_speed {
        control_flags_1 |= 0x08;
        packet[11] = fan.to_byte();
    }

    if let Some(vane) = wanted.vane {
        control_flags_1 |= 0x10;
        packet[12] = vane.to_byte();
    }

    if let Some(wide_vane) = wanted.wide_vane {
        control_flags_2 |= 0x01;
        packet[18] = wide_vane.to_byte();
    }

    packet[6] = control_flags_1;
    packet[7] = control_flags_2;

    packet[PACKET_LEN - 1] = calculate_checksum(&packet[..PACKET_LEN - 1]);
    packet
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::verify_checksum;

    #[test]
    fn test_connect_packet() {
        let p = build_connect_packet(false);
        assert!(verify_checksum(&p));
        assert_eq!(p[1], 0x5a);

        let p_inst = build_connect_packet(true);
        assert!(verify_checksum(&p_inst));
        assert_eq!(p_inst[1], 0x5b);
    }

    #[test]
    fn test_info_request_packet() {
        let p = build_info_request_packet(0x02);
        assert!(verify_checksum(&p));
        assert_eq!(p[0], 0xfc);
        assert_eq!(p[1], 0x42);
        assert_eq!(p[5], 0x02);
    }

    #[test]
    fn test_remote_temp_packet() {
        let p = build_remote_temp_packet(Some(22.5));
        assert!(verify_checksum(&p));
        assert_eq!(p[5], 0x07);
        assert_eq!(p[6], 0x01);
    }

    #[test]
    fn test_settings_packet() {
        let wanted = WantedSettings {
            power: Some(Power::On),
            mode: Some(Mode::Cool),
            temperature: Some(23.0),
            fan_speed: Some(FanSpeed::Speed3),
            vane: Some(VanePosition::Auto),
            wide_vane: Some(WideVane::Center),
        };
        let p = build_settings_packet(&wanted);
        assert!(verify_checksum(&p));
        assert_eq!(p[8], 0x01); // Power On
        assert_eq!(p[9], 0x03); // Mode Cool
        assert_eq!(p[11], 0x05); // Fan Speed 3
    }
}
