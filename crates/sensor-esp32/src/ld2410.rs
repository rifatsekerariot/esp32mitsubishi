//! HLK-LD2410 24GHz mmWave İnsan Varlığı ve Mikro Hareket Radarı Sürücüsü

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetType {
    NoTarget,
    MovingTarget,
    StationaryTarget, // Hareketsiz/Durağan (Nefes alma, oturma, uyuma)
    CombinedTarget,   // Hem hareketli hem hareketsiz hedef
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresenceData {
    pub presence_detected: bool,
    pub target_type: TargetType,
    pub moving_distance_cm: u16,
    pub moving_energy: u8,
    pub stationary_distance_cm: u16,
    pub stationary_energy: u8,
    pub detection_distance_cm: u16,
}

pub const FRAME_HEADER: [u8; 4] = [0xF4, 0xF3, 0xF2, 0xF1];
pub const FRAME_FOOTER: [u8; 4] = [0xF8, 0xF7, 0xF6, 0xF5];

pub struct Ld2410Parser {
    buffer: Vec<u8>,
}

impl Ld2410Parser {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(128),
        }
    }

    /// UART akışından gelen baytları tampona ekler ve tam bir paket bulduğunda çözümler
    pub fn push_bytes(&mut self, bytes: &[u8]) -> Option<PresenceData> {
        self.buffer.extend_from_slice(bytes);

        // 1. Başlığı ara
        while self.buffer.len() >= 4 {
            if self.buffer.starts_with(&FRAME_HEADER) {
                break;
            } else {
                self.buffer.remove(0);
            }
        }

        if self.buffer.len() < 16 {
            return None;
        }

        // 2. Footer'ı ara (en az 4 bayt sonra)
        let footer_pos = self.buffer.windows(4).enumerate().skip(4).find_map(|(idx, w)| {
            if w == FRAME_FOOTER {
                Some(idx)
            } else {
                None
            }
        });

        if let Some(pos) = footer_pos {
            let total_len = pos + 4;
            let frame: Vec<u8> = self.buffer.drain(0..total_len).collect();
            Self::parse_frame(&frame)
        } else {
            // Tampon çok büyüdüyse temizle
            if self.buffer.len() > 128 {
                self.buffer.remove(0);
            }
            None
        }
    }

    fn parse_frame(frame: &[u8]) -> Option<PresenceData> {
        // En az: 4 (header) + 2 (len) + 1 (data_type) + 1 (target_state) + 2 (mov_d) + 1 (mov_e) + 2 (stat_d) + 1 (stat_e) + 2 (det_d) + 4 (footer) = 20 bayt
        if frame.len() < 20 {
            return None;
        }

        let target_state = frame[7];
        let target_type = match target_state {
            0x01 => TargetType::MovingTarget,
            0x02 => TargetType::StationaryTarget,
            0x03 => TargetType::CombinedTarget,
            _ => TargetType::NoTarget,
        };

        let moving_distance_cm = u16::from_le_bytes([frame[8], frame[9]]);
        let moving_energy = frame[10];
        let stationary_distance_cm = u16::from_le_bytes([frame[11], frame[12]]);
        let stationary_energy = frame[13];
        let detection_distance_cm = u16::from_le_bytes([frame[14], frame[15]]);

        let presence_detected = target_type != TargetType::NoTarget;

        Some(PresenceData {
            presence_detected,
            target_type,
            moving_distance_cm,
            moving_energy,
            stationary_distance_cm,
            stationary_energy,
            detection_distance_cm,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ld2410_stationary_presence() {
        let mut parser = Ld2410Parser::new();

        // HLK-LD2410 gerçek durağan varlık paketi
        let raw = [
            0xF4, 0xF3, 0xF2, 0xF1,
            0x0B, 0x00,
            0x02,
            0x02, // Hedef: Stationary (Durağan insan / nefes alma)
            0x00, 0x00,
            0x00,
            0xC8, 0x00, // 200 cm mesafe
            0x55,       // 85 enerji
            0xC8, 0x00,
            0xF8, 0xF7, 0xF6, 0xF5,
        ];

        let res = parser.push_bytes(&raw).expect("Paket çözülmeli");
        assert!(res.presence_detected);
        assert_eq!(res.target_type, TargetType::StationaryTarget);
        assert_eq!(res.stationary_distance_cm, 200);
        assert_eq!(res.stationary_energy, 85);
    }
}
