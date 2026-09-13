//! Embedded SQLite database for historical temperature, humidity, and power telemetry.

use anyhow::Result;
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS telemetry_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                room_id TEXT NOT NULL,
                temperature REAL NOT NULL,
                humidity REAL NOT NULL,
                compressor_freq INTEGER NOT NULL,
                power_watts REAL NOT NULL
            );",
            [],
        )?;

        // Index on timestamp for rapid range queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_telemetry_ts ON telemetry_history (timestamp);",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert_point(
        &self,
        room_id: &str,
        temp: f32,
        humidity: f32,
        compressor_freq: u8,
        power_w: f32,
    ) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO telemetry_history (timestamp, room_id, temperature, humidity, compressor_freq, power_watts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![now, room_id, temp, humidity, compressor_freq, power_w],
        )?;
        Ok(())
    }
}
