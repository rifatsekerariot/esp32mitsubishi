//! Web Server, REST API, WebSocket handler and Embedded Static Assets.

use crate::state::{AppState, HvacUnit, SensorNode};
use axum::{
    extract::{
        path::Path,
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{header, HeaderMap, StatusCode, Uri},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;

#[derive(RustEmbed)]
#[folder = "../../ui/"]
pub struct Asset;

#[derive(Debug, Deserialize)]
pub struct ControlCommand {
    pub action: String,
    pub hvac_id: Option<String>,
    pub power: Option<String>,
    pub mode: Option<String>,
    pub temperature: Option<f32>,
    pub fan: Option<String>,
    pub vane: Option<String>,
    pub preset: Option<String>,
    pub assigned_sensor_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewHvacPayload {
    pub id: String,
    pub name: String,
    pub model: Option<String>,
    pub assigned_sensor_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewSensorPayload {
    pub id: String,
    pub name: String,
    pub room_zone: String,
    pub initial_temp: Option<f32>,
    pub initial_humidity: Option<f32>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/state", get(get_state))
        .route("/api/control", post(handle_control))
        .route("/api/hvac/register", post(register_hvac))
        .route("/api/hvac/:id", delete(delete_hvac))
        .route("/api/sensor/register", post(register_sensor))
        .route("/api/sensor/:id", delete(delete_sensor))
        .route("/ws", get(ws_handler))
        .fallback(static_handler)
        .with_state(Arc::new(state))
}

async fn get_state(State(app): State<Arc<AppState>>) -> impl IntoResponse {
    let state = app.home.read().await;
    Json((*state).clone())
}

async fn register_hvac(
    State(app): State<Arc<AppState>>,
    Json(payload): Json<NewHvacPayload>,
) -> impl IntoResponse {
    let mut state = app.home.write().await;
    let new_unit = HvacUnit {
        id: payload.id.clone(),
        name: payload.name,
        model: payload.model.unwrap_or_else(|| "Mitsubishi Electric CN105".to_string()),
        assigned_sensor_id: payload.assigned_sensor_id,
        power: "Off".to_string(),
        mode: "Heat".to_string(),
        target_temperature: 22.0,
        room_temperature: 21.5,
        outside_temperature: Some(15.4),
        fan_speed: "Auto".to_string(),
        vane: "Auto".to_string(),
        operating: false,
        compressor_frequency: 0,
        input_power_watts: 0.0,
        energy_kwh: 0.0,
        is_remote_temp_active: true,
        connected: true,
    };
    state.hvacs.push(new_unit);
    drop(state);
    app.notify_subscribers().await;
    Json(serde_json::json!({ "status": "ok", "id": payload.id }))
}

async fn delete_hvac(
    State(app): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut state = app.home.write().await;
    state.hvacs.retain(|h| h.id != id);
    if state.selected_hvac_id == id && !state.hvacs.is_empty() {
        state.selected_hvac_id = state.hvacs[0].id.clone();
    }
    drop(state);
    app.notify_subscribers().await;
    Json(serde_json::json!({ "status": "ok", "deleted": id }))
}

async fn register_sensor(
    State(app): State<Arc<AppState>>,
    Json(payload): Json<NewSensorPayload>,
) -> impl IntoResponse {
    let mut state = app.home.write().await;
    let new_node = SensorNode {
        id: payload.id.clone(),
        name: payload.name,
        room_zone: payload.room_zone,
        temperature: payload.initial_temp.unwrap_or(21.0),
        humidity: payload.initial_humidity.unwrap_or(50.0),
        presence: false,
        motion_energy: 0,
        battery: Some(100),
        rssi: -55,
        last_seen_secs: 0,
    };
    state.sensors.push(new_node);
    drop(state);
    app.notify_subscribers().await;
    Json(serde_json::json!({ "status": "ok", "id": payload.id }))
}

async fn delete_sensor(
    State(app): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut state = app.home.write().await;
    state.sensors.retain(|s| s.id != id);
    // Unassign if bound to an HVAC
    for hvac in &mut state.hvacs {
        if hvac.assigned_sensor_id.as_deref() == Some(&id) {
            hvac.assigned_sensor_id = None;
        }
    }
    drop(state);
    app.notify_subscribers().await;
    Json(serde_json::json!({ "status": "ok", "deleted": id }))
}

async fn handle_control(
    State(app): State<Arc<AppState>>,
    Json(cmd): Json<ControlCommand>,
) -> impl IntoResponse {
    info!("Processing control command: {:?}", cmd);
    let mut state = app.home.write().await;

    let target_hvac_id = cmd
        .hvac_id
        .clone()
        .unwrap_or_else(|| state.selected_hvac_id.clone());

    if let Some(hvac) = state.hvacs.iter_mut().find(|h| h.id == target_hvac_id) {
        match cmd.action.as_str() {
            "select_hvac" => {
                state.selected_hvac_id = target_hvac_id;
            }
            "set_power" => {
                if let Some(p) = cmd.power {
                    hvac.power = p;
                }
            }
            "set_temperature" => {
                if let Some(t) = cmd.temperature {
                    hvac.target_temperature = t;
                }
            }
            "set_mode" => {
                if let Some(m) = cmd.mode {
                    hvac.mode = m;
                    hvac.power = "On".to_string();
                }
            }
            "set_fan" => {
                if let Some(f) = cmd.fan {
                    hvac.fan_speed = f;
                }
            }
            "set_vane" => {
                if let Some(v) = cmd.vane {
                    hvac.vane = v;
                }
            }
            "assign_sensor" => {
                hvac.assigned_sensor_id = cmd.assigned_sensor_id.clone();
            }
            _ => {}
        }
    }

    if cmd.action == "set_preset" {
        if let Some(preset) = cmd.preset {
            state.global_mode = preset.clone();
            for hvac in &mut state.hvacs {
                match preset.as_str() {
                    "auto_comfort" => {
                        hvac.target_temperature = 22.0;
                        hvac.mode = "Auto".to_string();
                        hvac.fan_speed = "Auto".to_string();
                    }
                    "sleep" => {
                        hvac.target_temperature = 21.0;
                        hvac.mode = "Heat".to_string();
                        hvac.fan_speed = "Quiet".to_string();
                    }
                    "eco" => {
                        hvac.target_temperature = 20.0;
                        hvac.mode = "Heat".to_string();
                        hvac.fan_speed = "Speed1".to_string();
                    }
                    "away" => {
                        hvac.target_temperature = 17.0;
                        hvac.mode = "Heat".to_string();
                        hvac.fan_speed = "Quiet".to_string();
                    }
                    _ => {}
                }
            }
        }
    }

    drop(state);
    app.notify_subscribers().await;
    Json(serde_json::json!({ "status": "ok" }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(app): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

async fn handle_socket(mut socket: WebSocket, app: Arc<AppState>) {
    // 1. Send initial state immediately
    {
        let state = app.home.read().await;
        let payload = serde_json::json!({
            "type": "state_update",
            "payload": *state
        });
        if let Ok(json) = serde_json::to_string(&payload) {
            let _ = socket.send(Message::Text(json)).await;
        }
    }

    // 2. Subscribe to broadcasts
    let mut rx = app.ws_broadcast.subscribe();

    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                if socket.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
            Some(Ok(msg)) = socket.recv() => {
                if let Message::Text(text) = msg {
                    if let Ok(cmd) = serde_json::from_str::<ControlCommand>(&text) {
                        let app_clone = app.clone();
                        tokio::spawn(async move {
                            // Forward command processing
                            let mut state = app_clone.home.write().await;
                            let target_hvac_id = cmd.hvac_id.clone().unwrap_or_else(|| state.selected_hvac_id.clone());
                            if let Some(hvac) = state.hvacs.iter_mut().find(|h| h.id == target_hvac_id) {
                                if let Some(t) = cmd.temperature { hvac.target_temperature = t; }
                                if let Some(p) = cmd.power { hvac.power = p; }
                                if let Some(m) = cmd.mode { hvac.mode = m; }
                                if let Some(f) = cmd.fan { hvac.fan_speed = f; }
                                if let Some(v) = cmd.vane { hvac.vane = v; }
                            }
                            drop(state);
                            app_clone.notify_subscribers().await;
                        });
                    }
                }
            }
            else => break,
        }
    }
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match Asset::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, mime.as_ref().parse().unwrap());
            (StatusCode::OK, headers, content.data).into_response()
        }
        None => match Asset::get("index.html") {
            Some(content) => {
                let mut headers = HeaderMap::new();
                headers.insert(header::CONTENT_TYPE, "text/html".parse().unwrap());
                (StatusCode::OK, headers, content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        },
    }
}
