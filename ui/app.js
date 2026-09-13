// AuraClimate Industrial Core — Dynamic Multi-Device Orchestration Client
// Fully anti-slop, dynamic N-HVAC and M-Sensor node manager

let state = {
  selected_hvac_id: "ac_living_room",
  global_mode: "auto_comfort",
  hvacs: [],
  sensors: []
};

let ws = null;
let reconnectTimer = null;

// --- DOM REFERENCES ---
const dom = {
  hvacCountLabel: document.getElementById("hvacCountLabel"),
  sensorCountLabel: document.getElementById("sensorCountLabel"),
  totalPowerLabel: document.getElementById("totalPowerLabel"),
  commStatusBadge: document.getElementById("commStatusBadge"),
  commText: document.getElementById("commText"),
  liveClock: document.getElementById("liveClock"),
  hvacTabsContainer: document.getElementById("hvacTabsContainer"),
  hvacConsolePanel: document.getElementById("hvacConsolePanel"),
  sensorsGrid: document.getElementById("sensorsGrid"),
  orchestratorModeTag: document.getElementById("orchestratorModeTag"),
  diagInjectedRoom: document.getElementById("diagInjectedRoom"),
  globalPresetGroup: document.getElementById("globalPresetGroup"),
  
  // Modals
  addHvacModal: document.getElementById("addHvacModal"),
  openAddHvacModalBtn: document.getElementById("openAddHvacModalBtn"),
  closeAddHvacBtn: document.getElementById("closeAddHvacBtn"),
  cancelHvacModalBtn: document.getElementById("cancelHvacModalBtn"),
  addHvacForm: document.getElementById("addHvacForm"),
  newHvacSensorSelect: document.getElementById("newHvacSensor"),

  addSensorModal: document.getElementById("addSensorModal"),
  openAddSensorModalBtn: document.getElementById("openAddSensorModalBtn"),
  closeAddSensorBtn: document.getElementById("closeAddSensorBtn"),
  cancelSensorModalBtn: document.getElementById("cancelSensorModalBtn"),
  addSensorForm: document.getElementById("addSensorForm")
};

// --- INITIALIZATION ---
document.addEventListener("DOMContentLoaded", () => {
  initClock();
  initModalListeners();
  initGlobalPresetListeners();
  fetchInitialState();
  connectWebSocket();
});

// --- CLOCK ---
function initClock() {
  const update = () => {
    const now = new Date();
    dom.liveClock.textContent = now.toTimeString().split(" ")[0];
  };
  update();
  setInterval(update, 1000);
}

// --- DATA FETCHING & WEBSOCKET ---
async function fetchInitialState() {
  try {
    const res = await fetch("/api/state");
    if (res.ok) {
      state = await res.json();
      renderAll();
    }
  } catch (err) {
    console.warn("Initial REST fetch error, waiting for WebSocket:", err);
  }
}

function connectWebSocket() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const wsUrl = `${protocol}//${window.location.host}/ws`;

  try {
    ws = new WebSocket(wsUrl);

    ws.onopen = () => {
      dom.commStatusBadge.style.background = "var(--led-emerald-dim)";
      dom.commStatusBadge.style.borderColor = "rgba(0, 229, 153, 0.3)";
      dom.commText.textContent = "CANLI (PI ZERO HUB)";
      dom.commText.style.color = "var(--led-emerald)";
      clearTimeout(reconnectTimer);
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (msg.type === "state_update") {
          state = msg.payload;
          renderAll();
        }
      } catch (e) {
        console.error("WS parse error:", e);
      }
    };

    ws.onclose = () => {
      dom.commStatusBadge.style.background = "rgba(255, 75, 62, 0.15)";
      dom.commStatusBadge.style.borderColor = "rgba(255, 75, 62, 0.4)";
      dom.commText.textContent = "YENİDEN BAĞLANIYOR...";
      dom.commText.style.color = "var(--led-heat)";
      reconnectTimer = setTimeout(connectWebSocket, 3000);
    };

    ws.onerror = () => ws.close();
  } catch (err) {
    console.warn("WebSocket unavailable:", err);
  }
}

function sendCommand(payload) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(payload));
  } else {
    fetch("/api/control", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload)
    }).catch(e => console.error("REST control error:", e));
  }
}

// --- RENDER ALL SECTIONS ---
function renderAll() {
  renderTopStats();
  renderHvacTabs();
  renderActiveHvacConsole();
  renderSensorsGrid();
  renderDiagnostics();
  updateGlobalPresetsUI();
}

// 1. TOP STATS
function renderTopStats() {
  const activeCount = state.hvacs.filter(h => h.power === "On").length;
  dom.hvacCountLabel.textContent = `${String(activeCount).padStart(2, "0")} / ${String(state.hvacs.length).padStart(2, "0")} AKTİF`;
  dom.sensorCountLabel.textContent = `${String(state.sensors.length).padStart(2, "0")} BAĞLI`;

  const totalPower = state.hvacs.reduce((acc, h) => acc + (h.power === "On" ? h.input_power_watts : 0), 0);
  dom.totalPowerLabel.textContent = `${Math.round(totalPower)} W`;

  // Update sensor select options for add HVAC modal
  dom.newHvacSensorSelect.innerHTML = '<option value="">-- Otomatik Varlık Takibi --</option>';
  state.sensors.forEach(s => {
    const opt = document.createElement("option");
    opt.value = s.id;
    opt.textContent = `${s.name} (${s.room_zone})`;
    dom.newHvacSensorSelect.appendChild(opt);
  });
}

// 2. HVAC TABS RACK
function renderHvacTabs() {
  dom.hvacTabsContainer.innerHTML = "";

  state.hvacs.forEach(hvac => {
    const isSelected = hvac.id === state.selected_hvac_id;
    const btn = document.createElement("button");
    btn.className = `rack-tab-btn ${isSelected ? "active" : ""}`;
    btn.onclick = () => {
      state.selected_hvac_id = hvac.id;
      sendCommand({ action: "select_hvac", hvac_id: hvac.id });
      renderHvacTabs();
      renderActiveHvacConsole();
    };

    btn.innerHTML = `
      <span class="tab-status-led ${hvac.power === "On" ? "on" : ""}"></span>
      <span>${hvac.name}</span>
    `;

    dom.hvacTabsContainer.appendChild(btn);
  });
}

// 3. ACTIVE HVAC CONSOLE (LEFT PANEL)
function renderActiveHvacConsole() {
  const hvac = state.hvacs.find(h => h.id === state.selected_hvac_id) || state.hvacs[0];

  if (!hvac) {
    dom.hvacConsolePanel.innerHTML = `
      <div style="padding: 40px; text-align: center; color: var(--text-dim);">
        <p class="font-mono">SİSTEMDE TANIMLI KLİMA BULUNAMADI.</p>
        <button class="rack-action-btn" style="margin: 16px auto;" onclick="dom.addHvacModal.classList.add('open')">+ YENİ KLİMA EKLE</button>
      </div>
    `;
    return;
  }

  // Dial progress arc calculation (16°C to 31°C)
  const minT = 16.0;
  const maxT = 31.0;
  const pct = Math.max(0, Math.min(1, (hvac.target_temperature - minT) / (maxT - minT)));
  const strokeOffset = 480 - (pct * 320);

  const arcColor = hvac.mode === "Cool" ? "var(--led-cool)" : (hvac.mode === "Heat" ? "var(--led-heat)" : "var(--led-emerald)");

  const assignedSensor = state.sensors.find(s => s.id === hvac.assigned_sensor_id);
  const sensorDesc = assignedSensor ? `${assignedSensor.name} (${assignedSensor.room_zone})` : "Otomatik Varlık Takibi";

  dom.hvacConsolePanel.innerHTML = `
    <!-- Top info row -->
    <div class="console-top-row">
      <div class="console-unit-info">
        <h2>${hvac.name}</h2>
        <div class="console-unit-meta font-mono">
          <span>MODEL: ${hvac.model}</span>
          <span>•</span>
          <span>REFERANS: <b style="color:var(--text-pure);">${sensorDesc}</b></span>
        </div>
      </div>
      <div class="console-actions-group">
        <button class="mech-power-btn ${hvac.power === "On" ? "active" : ""}" id="activeHvacPowerBtn">
          <span class="power-led"></span>
          <span>${hvac.power === "On" ? "GÜÇ AÇIK" : "KAPALI"}</span>
        </button>
        <button class="bind-node-btn" style="color:var(--led-heat);" title="Klimayı Sistemden Kaldır" id="deleteHvacBtn">SİL</button>
      </div>
    </div>

    <!-- Gauge & Stepper -->
    <div class="dial-gauge-wrapper">
      <div class="dial-instrument">
        <svg class="dial-svg" viewBox="0 0 280 240">
          <path class="dial-track-arc" d="M 45,190 A 100,100 0 1,1 235,190" />
          <path class="dial-active-arc" style="stroke-dashoffset: ${strokeOffset}; stroke: ${arcColor};" d="M 45,190 A 100,100 0 1,1 235,190" />
        </svg>

        <div class="dial-readout-block">
          <div class="dial-temp-label font-mono">AKTİF ODA SICAKLIĞI</div>
          <div class="dial-primary-temp font-mono">${hvac.room_temperature.toFixed(1)}<small>°C</small></div>
          
          <div class="stepper-control-row">
            <button class="stepper-btn" id="hvacTempMinus">−</button>
            <div class="target-readout">
              <span class="target-title font-mono">HEDEF:</span>
              <span class="target-number" style="color: ${arcColor};">${hvac.target_temperature.toFixed(1)}</span>
              <span class="font-mono" style="font-size:0.75rem; color:var(--text-dim);">°C</span>
            </div>
            <button class="stepper-btn" id="hvacTempPlus">+</button>
          </div>
        </div>
      </div>
    </div>

    <!-- Mode, Fan, Vane Selectors -->
    <div class="instrument-selector-rack">
      <div class="selector-strip">
        <span class="selector-strip-label font-mono">ÇALIŞMA REJİMİ (MODE)</span>
        <div class="selector-button-bar" id="activeModeBar">
          <button class="inst-btn ${hvac.mode === "Heat" ? "active" : ""}" data-val="Heat">ISITMA</button>
          <button class="inst-btn ${hvac.mode === "Cool" ? "active" : ""}" data-val="Cool">SOĞUTMA</button>
          <button class="inst-btn ${hvac.mode === "Auto" ? "active" : ""}" data-val="Auto">OTOMATİK</button>
          <button class="inst-btn ${hvac.mode === "Dry" ? "active" : ""}" data-val="Dry">NEM ALMA</button>
          <button class="inst-btn ${hvac.mode === "Fan" ? "active" : ""}" data-val="Fan">FAN</button>
        </div>
      </div>

      <div class="selector-columns">
        <div class="selector-strip">
          <span class="selector-strip-label font-mono">FAN HIZI</span>
          <div class="selector-button-bar" id="activeFanBar">
            <button class="inst-btn ${hvac.fan_speed === "Auto" ? "active" : ""}" data-val="Auto">OTO</button>
            <button class="inst-btn ${hvac.fan_speed === "Quiet" ? "active" : ""}" data-val="Quiet">SESSİZ</button>
            <button class="inst-btn ${hvac.fan_speed === "Speed1" ? "active" : ""}" data-val="Speed1">1</button>
            <button class="inst-btn ${hvac.fan_speed === "Speed2" ? "active" : ""}" data-val="Speed2">2</button>
            <button class="inst-btn ${hvac.fan_speed === "Speed3" ? "active" : ""}" data-val="Speed3">3</button>
          </div>
        </div>

        <div class="selector-strip">
          <span class="selector-strip-label font-mono">HAVA KANADI (VANE)</span>
          <div class="selector-button-bar" id="activeVaneBar">
            <button class="inst-btn ${hvac.vane === "Auto" ? "active" : ""}" data-val="Auto">OTO</button>
            <button class="inst-btn ${hvac.vane === "Up" ? "active" : ""}" data-val="Up">YUKARI</button>
            <button class="inst-btn ${hvac.vane === "Center" ? "active" : ""}" data-val="Center">ORTA</button>
            <button class="inst-btn ${hvac.vane === "Down" ? "active" : ""}" data-val="Down">AŞAĞI</button>
            <button class="inst-btn ${hvac.vane === "Swing" ? "active" : ""}" data-val="Swing">SALINIM</button>
          </div>
        </div>
      </div>
    </div>

    <!-- Live Telemetry Strip -->
    <div class="live-telemetry-strip font-mono">
      <div class="telem-box">
        <span class="telem-label">KOMPRESÖR FREKANSI</span>
        <span class="telem-reading">${hvac.operating ? hvac.compressor_frequency + " Hz" : "BEKLEMEDE"}</span>
      </div>
      <div class="telem-box">
        <span class="telem-label">ANLIK GÜÇ TÜKETİMİ</span>
        <span class="telem-reading text-amber">${hvac.power === "On" ? Math.round(hvac.input_power_watts) + " W" : "0 W"}</span>
      </div>
      <div class="telem-box">
        <span class="telem-label">TOPLAM ENERJİ</span>
        <span class="telem-reading">${hvac.energy_kwh.toFixed(1)} kWh</span>
      </div>
    </div>
  `;

  // Bind Console Interactions
  document.getElementById("activeHvacPowerBtn").onclick = () => {
    const nextPower = hvac.power === "On" ? "Off" : "On";
    sendCommand({ action: "set_power", hvac_id: hvac.id, power: nextPower });
    hvac.power = nextPower;
    renderActiveHvacConsole();
  };

  document.getElementById("hvacTempMinus").onclick = () => {
    if (hvac.target_temperature > 16.0) {
      const newT = +(hvac.target_temperature - 0.5).toFixed(1);
      hvac.target_temperature = newT;
      sendCommand({ action: "set_temperature", hvac_id: hvac.id, temperature: newT });
      renderActiveHvacConsole();
    }
  };

  document.getElementById("hvacTempPlus").onclick = () => {
    if (hvac.target_temperature < 31.0) {
      const newT = +(hvac.target_temperature + 0.5).toFixed(1);
      hvac.target_temperature = newT;
      sendCommand({ action: "set_temperature", hvac_id: hvac.id, temperature: newT });
      renderActiveHvacConsole();
    }
  };

  document.querySelectorAll("#activeModeBar .inst-btn").forEach(btn => {
    btn.onclick = () => {
      const mode = btn.dataset.val;
      hvac.mode = mode;
      hvac.power = "On";
      sendCommand({ action: "set_mode", hvac_id: hvac.id, mode });
      renderActiveHvacConsole();
    };
  });

  document.querySelectorAll("#activeFanBar .inst-btn").forEach(btn => {
    btn.onclick = () => {
      const fan = btn.dataset.val;
      hvac.fan_speed = fan;
      sendCommand({ action: "set_fan", hvac_id: hvac.id, fan });
      renderActiveHvacConsole();
    };
  });

  document.querySelectorAll("#activeVaneBar .inst-btn").forEach(btn => {
    btn.onclick = () => {
      const vane = btn.dataset.val;
      hvac.vane = vane;
      sendCommand({ action: "set_vane", hvac_id: hvac.id, vane });
      renderActiveHvacConsole();
    };
  });

  document.getElementById("deleteHvacBtn").onclick = async () => {
    if (confirm(`'${hvac.name}' cihazını sistemden kaldırmak istediğinize emin misiniz?`)) {
      await fetch(`/api/hvac/${hvac.id}`, { method: "DELETE" });
      fetchInitialState();
    }
  };
}

// 4. SENSORS MATRIX (RIGHT PANEL)
function renderSensorsGrid() {
  dom.sensorsGrid.innerHTML = "";

  const activeHvac = state.hvacs.find(h => h.id === state.selected_hvac_id);

  state.sensors.forEach(sensor => {
    const isBound = activeHvac && activeHvac.assigned_sensor_id === sensor.id;
    const card = document.createElement("div");
    card.className = `sensor-node-card ${isBound ? "bound-to-active" : ""}`;

    card.innerHTML = `
      <div class="node-card-top">
        <div class="node-title-group">
          <span class="node-zone-name">${sensor.name}</span>
          <span class="node-hardware-id font-mono">${sensor.id} • ${sensor.room_zone}</span>
        </div>
        <div class="radar-motion-indicator ${sensor.presence ? "active" : ""}">
          <span class="radar-dot"></span>
          <span>${sensor.presence ? `VARLIK %${sensor.motion_energy}` : "BOŞ"}</span>
        </div>
      </div>

      <div class="node-readouts">
        <div class="node-temp-display font-mono">${sensor.temperature.toFixed(1)}<small>°C</small></div>
        <div class="node-humidity-display font-mono">%${sensor.humidity.toFixed(0)} RH</div>
      </div>

      <div class="node-footer-bar font-mono">
        <span>${sensor.battery ? `PIL %${sensor.battery}` : "AC GÜÇ"} • ${sensor.rssi} dBm</span>
        <div style="display:flex; gap:6px;">
          <button class="bind-node-btn ${isBound ? "bound" : ""}" onclick="bindSensorToActiveHvac('${sensor.id}')">
            ${isBound ? "BAĞLI" : "BU KLİMAYA BAĞLA"}
          </button>
          <button class="bind-node-btn" style="color:var(--led-heat);" onclick="deleteSensor('${sensor.id}')">SİL</button>
        </div>
      </div>
    `;

    dom.sensorsGrid.appendChild(card);
  });
}

function bindSensorToActiveHvac(sensorId) {
  const activeHvac = state.hvacs.find(h => h.id === state.selected_hvac_id);
  if (activeHvac) {
    activeHvac.assigned_sensor_id = sensorId;
    sendCommand({
      action: "assign_sensor",
      hvac_id: activeHvac.id,
      assigned_sensor_id: sensorId
    });
    renderAll();
  }
}

async function deleteSensor(sensorId) {
  if (confirm(`'${sensorId}' sensörünü sistemden kaldırmak istiyor musunuz?`)) {
    await fetch(`/api/sensor/${sensorId}`, { method: "DELETE" });
    fetchInitialState();
  }
}

// 5. DIAGNOSTICS & TELEMETRY
function renderDiagnostics() {
  const activeHvac = state.hvacs.find(h => h.id === state.selected_hvac_id);
  if (activeHvac) {
    const boundSensor = state.sensors.find(s => s.id === activeHvac.assigned_sensor_id);
    if (boundSensor) {
      dom.diagInjectedRoom.textContent = `${boundSensor.name} (${boundSensor.temperature.toFixed(1)}°C) → ${activeHvac.name}`;
      dom.diagInjectedRoom.className = "diag-val font-mono text-cool";
    } else {
      dom.diagInjectedRoom.textContent = "Dinamik Varlık Arama Modu";
      dom.diagInjectedRoom.className = "diag-val font-mono text-amber";
    }
  }
}

// 6. GLOBAL PRESETS
function initGlobalPresetListeners() {
  dom.globalPresetGroup.querySelectorAll(".preset-switch").forEach(btn => {
    btn.onclick = () => {
      const preset = btn.dataset.preset;
      state.global_mode = preset;
      sendCommand({ action: "set_preset", preset });
      updateGlobalPresetsUI();
    };
  });
}

function updateGlobalPresetsUI() {
  dom.globalPresetGroup.querySelectorAll(".preset-switch").forEach(btn => {
    btn.classList.toggle("active", btn.dataset.preset === state.global_mode);
  });
  dom.orchestratorModeTag.textContent = state.global_mode.toUpperCase().replace("_", "-");
}

// 7. MODALS (ADD HVAC & ADD SENSOR)
function initModalListeners() {
  // Open / Close Add HVAC
  dom.openAddHvacModalBtn.onclick = () => dom.addHvacModal.classList.add("open");
  dom.closeAddHvacBtn.onclick = () => dom.addHvacModal.classList.remove("open");
  dom.cancelHvacModalBtn.onclick = () => dom.addHvacModal.classList.remove("open");

  dom.addHvacForm.onsubmit = async (e) => {
    e.preventDefault();
    const id = document.getElementById("newHvacId").value.trim();
    const name = document.getElementById("newHvacName").value.trim();
    const model = document.getElementById("newHvacModel").value.trim();
    const sensor = dom.newHvacSensorSelect.value || null;

    if (id && name) {
      await fetch("/api/hvac/register", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id, name, model, assigned_sensor_id: sensor })
      });
      dom.addHvacModal.classList.remove("open");
      dom.addHvacForm.reset();
      fetchInitialState();
    }
  };

  // Open / Close Add Sensor
  dom.openAddSensorModalBtn.onclick = () => dom.addSensorModal.classList.add("open");
  dom.closeAddSensorBtn.onclick = () => dom.addSensorModal.classList.remove("open");
  dom.cancelSensorModalBtn.onclick = () => dom.addSensorModal.classList.remove("open");

  dom.addSensorForm.onsubmit = async (e) => {
    e.preventDefault();
    const id = document.getElementById("newSensorId").value.trim();
    const name = document.getElementById("newSensorName").value.trim();
    const zone = document.getElementById("newSensorZone").value.trim();

    if (id && name && zone) {
      await fetch("/api/sensor/register", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id, name, room_zone: zone })
      });
      dom.addSensorModal.classList.remove("open");
      dom.addSensorForm.reset();
      fetchInitialState();
    }
  };
}
