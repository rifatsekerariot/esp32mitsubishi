//! CN105 Temperature encoding and decoding functions.

/// Decode temperature from raw encoding bytes.
///
/// In the CN105 protocol:
/// - Encoding B (half-degree precision): When `enc_b != 0`, temperature is `(enc_b - 128) / 2.0`
/// - Encoding A (integer precision): When `enc_b == 0`, temperature is `enc_a + offset` (default offset = 10 for room temp).
#[inline]
pub fn decode_temperature(enc_a: u8, enc_b: u8, offset: i32) -> f32 {
    if enc_b != 0 {
        (enc_b as f32 - 128.0) / 2.0
    } else {
        enc_a as f32 + offset as f32
    }
}

/// Encode a target setpoint temperature in °C into the standard Encoding B format.
/// Formula: `round(temp * 2.0) + 128`
#[inline]
pub fn encode_temperature_b(temperature: f32) -> u8 {
    let clamped = temperature.clamp(10.0, 31.5);
    ((clamped * 2.0).round() as i32 + 128) as u8
}

/// Encode a remote temperature into the two-byte format used by remote temp packets (SET 0x07).
/// - `enc_a`: `round(temp * 2.0) - 16`
/// - `enc_b`: `round(temp * 2.0) + 128`
#[inline]
pub fn encode_remote_temperature(temperature: f32) -> (u8, u8) {
    let clamped = temperature.clamp(10.0, 35.0);
    let half_steps = (clamped * 2.0).round() as i32;
    let enc_a = (half_steps - 16) as u8;
    let enc_b = (half_steps + 128) as u8;
    (enc_a, enc_b)
}

/// Decode an MSZ-A24NA setpoint byte into °C.
#[inline]
pub fn decode_msz_a24na_setpoint(byte: u8) -> f32 {
    31.0 - (byte & 0x0F) as f32 + 0.5 * (((byte >> 4) & 0x01) as f32)
}

/// Encode a target temperature into MSZ-A24NA setpoint format.
#[inline]
pub fn encode_msz_a24na_setpoint(temperature: f32) -> u8 {
    let clamped = temperature.clamp(16.0, 31.0);
    let half_steps = ((clamped - 16.0) * 2.0).round() as u8;
    let low = 15 - (half_steps / 2);
    let high = (half_steps % 2) << 4;
    high | low
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_encoding_decoding() {
        let temp = 22.5f32;
        let enc_b = encode_temperature_b(temp);
        assert_eq!(enc_b, 173); // 22.5 * 2 + 128 = 45 + 128 = 173
        let decoded = decode_temperature(0, enc_b, 10);
        assert_eq!(decoded, 22.5);
    }

    #[test]
    fn test_remote_temperature_encoding() {
        let (a, b) = encode_remote_temperature(21.0);
        // 21.0 * 2 = 42
        // a = 42 - 16 = 26
        // b = 42 + 128 = 170
        assert_eq!(a, 26);
        assert_eq!(b, 170);
    }
}
