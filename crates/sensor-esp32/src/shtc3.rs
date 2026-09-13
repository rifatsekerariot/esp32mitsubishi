//! Sensirion SHTC3 / AHT20 I2C Sıcaklık ve Nem Sensör Sürücüsü

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SensorReading {
    pub temperature: f32,
    pub humidity: f32,
}

pub const SHTC3_I2C_ADDR: u8 = 0x70;

// SHTC3 Komut Kodları
pub const CMD_WAKEUP: [u8; 2] = [0x35, 0x17];
pub const CMD_SLEEP: [u8; 2] = [0xB0, 0x98];
pub const CMD_MEASURE_NORMAL: [u8; 2] = [0x78, 0x66]; // Normal mod, Temp ilk, Clock stretch kapalı

/// Sensirion standart CRC-8 hesaplayıcı (Polinom: 0x31, Init: 0xFF)
pub fn check_crc(data: &[u8], expected_crc: u8) -> bool {
    let mut crc: u8 = 0xFF;
    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            if (crc & 0x80) != 0 {
                crc = (crc << 1) ^ 0x31;
            } else {
                crc <<= 1;
            }
        }
    }
    crc == expected_crc
}

/// SHTC3 sensöründen okunan 6 baytlık ham veriyi sıcaklık ve neme çevirir
/// Format: [T_MSB, T_LSB, T_CRC, RH_MSB, RH_LSB, RH_CRC]
pub fn parse_shtc3_data(buf: &[u8; 6]) -> Result<SensorReading, &'static str> {
    // CRC Doğrulama
    if !check_crc(&buf[0..2], buf[2]) {
        return Err("Sıcaklık CRC hatası");
    }
    if !check_crc(&buf[3..5], buf[5]) {
        return Err("Nem CRC hatası");
    }

    let raw_temp = u16::from_be_bytes([buf[0], buf[1]]) as f32;
    let raw_hum = u16::from_be_bytes([buf[3], buf[4]]) as f32;

    // Sensirion SHTC3 dönüşüm formülleri:
    // T = -45 + 175 * (S_T / 2^16)
    // RH = 100 * (S_RH / 2^16)
    let temperature = -45.0 + (175.0 * raw_temp / 65535.0);
    let humidity = (100.0 * raw_hum / 65535.0).clamp(0.0, 100.0);

    Ok(SensorReading {
        temperature: (temperature * 10.0).round() / 10.0,
        humidity: (humidity * 10.0).round() / 10.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_and_parsing() {
        // Örnek ham veri: T=21.5°C, RH=45.0% için hesaplanmış test dizisi
        // raw_temp ~ 24907 (0x614B), raw_hum ~ 29491 (0x7333)
        let t_bytes = [0x61, 0x4B];
        let mut crc_t: u8 = 0xFF;
        for &b in &t_bytes {
            crc_t ^= b;
            for _ in 0..8 {
                crc_t = if (crc_t & 0x80) != 0 { (crc_t << 1) ^ 0x31 } else { crc_t << 1 };
            }
        }

        let rh_bytes = [0x73, 0x33];
        let mut crc_rh: u8 = 0xFF;
        for &b in &rh_bytes {
            crc_rh ^= b;
            for _ in 0..8 {
                crc_rh = if (crc_rh & 0x80) != 0 { (crc_rh << 1) ^ 0x31 } else { crc_rh << 1 };
            }
        }

        let packet = [t_bytes[0], t_bytes[1], crc_t, rh_bytes[0], rh_bytes[1], crc_rh];
        let reading = parse_shtc3_data(&packet).expect("Ayrıştırma başarılı olmalı");

        assert!(reading.temperature > 20.0 && reading.temperature < 23.0);
        assert!(reading.humidity > 44.0 && reading.humidity < 46.0);
    }
}
