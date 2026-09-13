mod controller;

use controller::HvacController;
use std::thread::sleep;
use std::time::Duration;

#[cfg(not(target_os = "espidf"))]
fn main() {
    println!("--- Mitsubishi CN105 Rust HVAC Controller (Desktop / Sim Mode) ---");
    let mut ctrl = HvacController::new();

    // Demonstrate state machine and packet generation
    if let Some(pkt) = ctrl.get_next_outgoing_packet() {
        println!("Generated connect packet: {:02X?}", pkt);
    }

    // Simulate heatpump ACK (0x7A)
    let connect_reply = [0xFC, 0x7A, 0x01, 0x30, 0x02, 0xCA, 0x01, 0x88];
    for b in connect_reply {
        ctrl.on_uart_byte(b);
    }
    println!("Is connected: {}", ctrl.is_connected);

    let state = ctrl.create_published_state();
    println!("Published state JSON: {}", serde_json::to_string_pretty(&state).unwrap());
}

#[cfg(target_os = "espidf")]
fn main() {
    use esp_idf_svc::hal::prelude::*;
    use esp_idf_svc::hal::uart::*;
    use esp_idf_svc::log::EspLogger;

    EspLogger::initialize_default();
    log::info!("Starting Mitsubishi CN105 Rust HVAC Controller on ESP32...");

    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    // Configure UART2 for CN105: 2400 baud, 8N1/8E1
    // TX = GPIO17, RX = GPIO16 (standard ESP32 DevKit pinout)
    let config = config::Config::new().baudrate(2400.into());
    let uart = UartDriver::new(
        peripherals.uart2,
        pins.gpio17, // TX
        pins.gpio16, // RX
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        &config,
    )
    .unwrap();

    let mut controller = HvacController::new();
    let mut rx_buf = [0u8; 64];

    loop {
        // 1. Read incoming bytes from CN105 UART
        if let Ok(bytes_read) = uart.read(&mut rx_buf, 10) {
            for &byte in &rx_buf[..bytes_read] {
                controller.on_uart_byte(byte);
            }
        }

        // 2. Transmit any scheduled packets (queries, settings updates, remote temp)
        if let Some(pkt) = controller.get_next_outgoing_packet() {
            let _ = uart.write(&pkt);
        }

        sleep(Duration::from_millis(10));
    }
}
