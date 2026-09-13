//! ESP32 Çoklu Oda Ortam ve Varlık Sensörü (Multi-Room Environment & Presence Sensor Node)
//! Sensirion SHTC3 (I2C) & HLK-LD2410 24GHz mmWave Radar (UART)

pub mod ld2410;
pub mod shtc3;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomTelemetry {
    pub room_id: String,
    pub room_name: String,
    pub temperature: f32,
    pub humidity: f32,
    pub presence_detected: bool,
    pub motion_energy: u8,
    pub distance_cm: Option<u16>,
    pub battery_level: Option<u8>,
}

#[cfg(not(target_os = "espidf"))]
fn main() {
    println!("=======================================================");
    println!("  ESP32 ODA SENSÖR DÜĞÜMÜ (Masaüstü Test / Simülasyon)  ");
    println!("  Donanım: SHTC3 (I2C) + HLK-LD2410 24GHz mmWave Radar ");
    println!("=======================================================");

    let mut ld2410_parser = ld2410::Ld2410Parser::new();

    // Simülasyon radar paketi (Salonda koltukta hareketsiz oturan kişi algılandı)
    let sample_radar_frame = [
        0xF4, 0xF3, 0xF2, 0xF1,
        0x0D, 0x00,
        0x02,
        0x02, // Hareketsiz durağan insan varlığı
        0x00, 0x00, 0x00,
        0x96, 0x00, // 150 cm mesafe
        0x50,       // %80 varlık enerjisi
        0x96, 0x00,
        0x00,
        0xF8, 0xF7, 0xF6, 0xF5,
    ];

    let presence = ld2410_parser
        .push_bytes(&sample_radar_frame)
        .expect("Radar paketi ayrıştırılamadı");

    let telemetry = RoomTelemetry {
        room_id: "living_room".to_string(),
        room_name: "Salon".to_string(),
        temperature: 23.4,
        humidity: 48.2,
        presence_detected: presence.presence_detected,
        motion_energy: presence.stationary_energy,
        distance_cm: Some(presence.stationary_distance_cm),
        battery_level: Some(100),
    };

    println!("\n✅ Sensör Okumaları Başarılı:");
    println!("   - Sıcaklık : {} °C", telemetry.temperature);
    println!("   - Nem      : %{}", telemetry.humidity);
    println!("   - İnsan Varlığı: {}", if telemetry.presence_detected { "ALGILANDI (Koltukta Oturuyor)" } else { "Kimse Yok" });
    println!("   - Varlık Enerjisi: %{}", telemetry.motion_energy);
    println!("   - Mesafe: {} cm", telemetry.distance_cm.unwrap_or(0));

    println!("\n📡 Core-Hub'a Gönderilecek JSON Yükü (POST /api/sensors):");
    println!("{}", serde_json::to_string_pretty(&telemetry).unwrap());
}

#[cfg(target_os = "espidf")]
fn main() {
    use esp_idf_svc::log::EspLogger;
    use esp_idf_svc::sys::link_patches;
    use esp_idf_hal::peripherals::Peripherals;
    use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
    use esp_idf_hal::uart::{UartConfig, UartDriver};
    use esp_idf_hal::units::Hertz;

    link_patches();
    EspLogger::initialize_default();
    log::info!("ESP32 Oda Sensör Düğümü Başlatılıyor (SHTC3 + LD2410)...");

    let peripherals = Peripherals::take().unwrap();

    // 1. I2C Başlatma (SHTC3 Sensörü - SDA: GPIO 21, SCL: GPIO 22)
    let i2c_config = I2cConfig::new().baudrate(Hertz(100_000));
    let mut i2c = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
        &i2c_config,
    ).expect("I2C driver başlatılamadı");

    // 2. UART Başlatma (HLK-LD2410 Radar - 256000 Baud, TX: GPIO 17, RX: GPIO 16)
    let uart_config = UartConfig::new().baudrate(Hertz(256_000));
    let uart = UartDriver::new(
        peripherals.uart1,
        peripherals.pins.gpio17,
        peripherals.pins.gpio16,
        Option::<esp_idf_hal::gpio::AnyIOPin>::None,
        Option::<esp_idf_hal::gpio::AnyIOPin>::None,
        &uart_config,
    ).expect("UART driver başlatılamadı");

    let mut radar_parser = ld2410::Ld2410Parser::new();
    let room_id = "living_room";
    let room_name = "Salon";

    log::info!("Sensör döngüsü aktif. 5 saniyede bir ölçüm ve telemetri yayını yapılacak.");

    loop {
        // --- SHTC3 Sıcaklık & Nem Ölçümü ---
        let mut temp = 22.5;
        let mut hum = 50.0;

        // Sensörü uyandır
        let _ = i2c.write(shtc3::SHTC3_I2C_ADDR, &shtc3::CMD_WAKEUP, 100);
        sleep(Duration::from_millis(1));

        // Ölçüm komutu gönder
        if i2c.write(shtc3::SHTC3_I2C_ADDR, &shtc3::CMD_MEASURE_NORMAL, 100).is_ok() {
            sleep(Duration::from_millis(15)); // Ölçüm dönüş süresi
            let mut buf = [0u8; 6];
            if i2c.read(shtc3::SHTC3_I2C_ADDR, &mut buf, 100).is_ok() {
                if let Ok(reading) = shtc3::parse_shtc3_data(&buf) {
                    temp = reading.temperature;
                    hum = reading.humidity;
                }
            }
        }
        // Sensörü uyku moduna al (Düşük güç tüketimi)
        let _ = i2c.write(shtc3::SHTC3_I2C_ADDR, &shtc3::CMD_SLEEP, 100);

        // --- HLK-LD2410 Radar Okuması ---
        let mut uart_buf = [0u8; 64];
        let mut presence_detected = false;
        let mut motion_energy = 0;
        let mut distance_cm = None;

        if let Ok(len) = uart.read(&mut uart_buf, 50) {
            if len > 0 {
                if let Some(presence) = radar_parser.push_bytes(&uart_buf[..len]) {
                    presence_detected = presence.presence_detected;
                    motion_energy = presence.moving_energy.max(presence.stationary_energy);
                    distance_cm = Some(presence.detection_distance_cm);
                }
            }
        }

        let telemetry = RoomTelemetry {
            room_id: room_id.to_string(),
            room_name: room_name.to_string(),
            temperature: temp,
            humidity: hum,
            presence_detected,
            motion_energy,
            distance_cm,
            battery_level: None,
        };

        log::info!("Telemetri: T={:.1}°C, H={:.1}%, Varlık={}", temp, hum, presence_detected);

        // HTTP POST ile core-hub'a telemetri gönder (http://<hub-ip>:8080/api/sensors)
        // ... (EspHttpConnection ile entegre edilir)

        sleep(Duration::from_secs(5));
    }
}
