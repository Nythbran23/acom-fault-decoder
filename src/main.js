// ============================================================================
// main.js — ACOM Fault Decoder frontend logic.
//
// Uses window.__TAURI__ (injected by Tauri when withGlobalTauri=true).
// invoke() replaces window.electronAPI.X()
// listen()  replaces window.electronAPI.onX()
// ============================================================================

const { invoke } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

// ── Sample signature (from Excel verification capture) ───────────────────────
const SAMPLE_LINES = [
    "0000 0000 0002 0000 0000 0000 0000 0000 0000 0000 0000 0041 0040 0000 0863 F71A",
    "0000 0000 0000 0000 0000 00FC 13A0 01FE 0000 0000 0000 0129 0000 0000 0000 E83D",
    "0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0120 0000 01F4 FCEC",
    "0000 0000 0000 0000 0000 0000 0000 0000 003E 0000 0000 8059 0080 0104 0291 7B54"
];

// ── State ────────────────────────────────────────────────────────────────────
let isConnected = false;
let captureCounter = 0;
let signatureHistory = [];
let lastResult = null;

// ── DOM refs ─────────────────────────────────────────────────────────────────
const $ = id => document.getElementById(id);

// ============================================================================
// Initialisation
// ============================================================================
window.addEventListener('DOMContentLoaded', async () => {
    // Pull version from Tauri app metadata
    try {
        const version = await invoke('get_app_version');
        const el = document.getElementById('appVersion');
        if (el) el.textContent = 'v' + version;
    } catch(e) { console.warn('version fetch failed:', e); }

    await refreshPorts();

    // 500S / Global controls
    $('refreshPortsBtn').addEventListener('click', refreshPorts);
    $('connectSerialBtn').addEventListener('click', connectSerial);
    $('disconnectSerialBtn').addEventListener('click', disconnectSerial);
    $('decodeBtn').addEventListener('click', decodeManual);
    $('loadFileBtn').addEventListener('click', loadSignatureFile);
    $('sampleBtn').addEventListener('click', loadSample);
    $('clearBtn').addEventListener('click', clearAll);
    // clearHistoryBtn removed
    $('loadFileBtn').addEventListener('click', loadSignatureFile);
    $('sampleBtn').addEventListener('click', loadSample);
    // openFolderBtn removed

    // Legacy controls
    $('legacyDecodeBtn').addEventListener('click', decodeLegacy);
    $('legacySampleBtn').addEventListener('click', loadLegacySample);
    $('legacyClearBtn').addEventListener('click', clearLegacy);

    // Legacy auto-advance and live checksum
    const legacyLengths = { lg1: 6, lg2: 6, lg3: 6, lg4: 6, lg5: 6, lg6: 6 };
    const legacyOrder  = ['lg1','lg2','lg3','lg4','lg5','lg6'];
    legacyOrder.forEach((id, idx) => {
        $(id).addEventListener('input', () => {
            autoUpperLegacy(id);
            // Auto-advance when field is full
            const max = legacyLengths[id];
            if ($(id).value.replace(/\s/g,'').length >= max && idx < legacyOrder.length - 1) {
                $(legacyOrder[idx + 1]).focus();
                $(legacyOrder[idx + 1]).select();
            }
            validateLegacyChecksum();
        });
    });

    for (let i = 1; i <= 4; i++) {
        $(`line${i}`).addEventListener('input',   () => autoSpaceHex(i));
        $(`line${i}`).addEventListener('change',  () => validateLineChecksum(i));
    }

    // Tauri event listeners
    await listen('serial-line-received', e => onLineReceived(e.payload));
    await listen('serial-signature-complete', e => onSignatureComplete(e.payload.lines));
    await listen('serial-error', e => { setStatus('⚠ Serial error: ' + e.payload); updateSerialStatus('Error', '#e74c3c'); });
    await listen('serial-disconnected', () => onDisconnected());
});

// ============================================================================
// Port management
// ============================================================================
async function refreshPorts() {
    const ports = await invoke('list_serial_ports');
    const sel = $('portSelect');
    sel.innerHTML = '<option value="">— select port —</option>';
    ports.forEach(p => {
        const opt = document.createElement('option');
        opt.value = p.path;
        const label = [p.path, p.manufacturer, p.product].filter(Boolean).join(' — ');
        opt.textContent = label;
        sel.appendChild(opt);
    });
    if (ports.length === 1) sel.value = ports[0].path;
}

async function connectSerial() {
    const port = $('portSelect').value;
    if (!port) { setStatus('⚠ Select a port first'); return; }
    updateSerialStatus('Connecting…', '#f39c12');
    try {
        await invoke('connect_serial', { port });
        isConnected = true;
        $('connectSerialBtn').style.display = 'none';
        $('disconnectSerialBtn').style.display = '';
        $('serialCapture').style.display = '';
        updateSerialStatus('Connected: ' + port, '#27ae60');
        setStatus('Connected to ' + port);
        updateCaptureStatus('⏳ Waiting for ACOM data…<br>Set amp: MENU → FAULTS LOG');
    } catch (err) {
        updateSerialStatus('Failed', '#e74c3c');
        setStatus('⚠ Connection failed: ' + err);
    }
}

async function disconnectSerial() {
    await invoke('disconnect_serial');
    onDisconnected();
}

function onDisconnected() {
    isConnected = false;
    $('connectSerialBtn').style.display = '';
    $('disconnectSerialBtn').style.display = 'none';
    $('serialCapture').style.display = 'none';
    updateSerialStatus('Not connected', '#7f8c8d');
    setStatus('Disconnected');
}

// ============================================================================
// Serial capture callbacks
// ============================================================================
function onLineReceived({ line_number, line }) {
    const prog = $('captureProgress');
    const div = document.createElement('div');
    div.style.cssText = 'margin-top:4px; color:#27ae60; font-size:11px;';
    div.textContent = `✓ Line ${line_number}/4`;
    prog.appendChild(div);
}

async function onSignatureComplete(lines) {
    captureCounter++;
    const ts = new Date().toISOString().replace(/[:.]/g, '-').substring(0, 19);
    const filename = `acom_sig_${captureCounter}_${ts}.txt`;
    const data = buildTextCapture(captureCounter, lines);

    try {
        await invoke('auto_save_signature', { data, filename });
    } catch (e) { console.warn('Auto-save failed:', e); }

    addToHistory({ id: captureCounter, timestamp: new Date(), filename, lines });

    for (let i = 0; i < 4; i++) { $(`line${i+1}`).value = lines[i]; autoSpaceHex(i+1); }
    updateCaptureStatus(`✅ Captured #${captureCounter} — decoding…`);
    $('captureProgress').innerHTML = '';

    setTimeout(async () => {
        await decodeLines(lines);
        updateCaptureStatus(`⏳ Waiting… (${captureCounter} captured)`);
    }, 400);
}

// ============================================================================
// Manual decode (500S / 600S / 700S)
// ============================================================================
async function decodeManual() {
    const lines = [1,2,3,4].map(i => $(`line${i}`).value.trim());
    if (lines.some(l => !l)) { setStatus('⚠ All 4 lines required'); return; }
    await decodeLines(lines);
}

async function decodeLines(lines) {
    setStatus('Decoding…');
    try {
        const response = await invoke('decode_signature', { lines });
        lastResult = response;
        renderResults(response.signature, response.diagnosis, lines);
    } catch (err) {
        $('results').innerHTML = `<div class="fault-hard" style="padding:20px">⚠ Decode error: ${err}</div>`;
        setStatus('⚠ Decode failed: ' + err);
    }
}

// ============================================================================
// Render results (500S Family)
// ============================================================================
function renderResults(sig, diagnosis, rawLines) {
    const faults = sig.active_faults ?? [];
    const hard  = faults.filter(f => f.fault_type === 'HardFault');
    const soft  = faults.filter(f => f.fault_type === 'SoftFault');
    const warn  = faults.filter(f => f.fault_type === 'Warning');

    let html = '';
    html += `<div class="decode-header">═══════════════════════════════════════════════════════════════\n`;
    html += `  ACOM HARD FAULT SIGNATURE DECODE  —  ${new Date().toLocaleString()}\n`;
    html += `═══════════════════════════════════════════════════════════════</div>`;

    const clock = sig.clock;
    html += card('Amplifier State', [
        ['Mode',         decodeAmpMode(sig.amp_mode)],
        ['Jump State',   `0x${sig.jump_state.toString(16).toUpperCase().padStart(4,'0')}`],
        ['Working Time', `${clock.hours}h ${String(clock.minutes).padStart(2,'0')}m ${String(clock.seconds).padStart(2,'0')}s`],
    ]);

    html += `<hr class="decode-separator">`;
    html += `<div style="color:#bdc3c7; font-weight:700; margin-bottom:8px;">ACTIVE FAULT CODES</div>`;
    if (faults.length === 0) {
        html += `<div class="fault-warning" style="padding:8px">✓ No active fault codes</div>`;
    } else {
        faults.forEach(f => {
            const cls = f.fault_type === 'HardFault' ? 'fault-hard' : f.fault_type === 'SoftFault' ? 'fault-soft' : 'fault-warning';
            const badge = f.fault_type === 'HardFault' ? '⛔ HF' : f.fault_type === 'SoftFault' ? '⚠ SF' : '● WRN';
            html += `<div class="fault-item"><span class="${cls}">${badge} &nbsp; 0x${f.code.toString(16).toUpperCase().padStart(2,'0')}</span> &nbsp; ${f.condition}</div>`;
        });
    }

    html += renderDiagnosis(diagnosis);

    const rf = sig.rf;
    html += card('RF Parameters', [
        ['Frequency',       `${(rf.frequency_khz/1000).toFixed(3)} MHz`],
        ['Forward Power',   `${rf.forward_power_w.toFixed(1)} W`],
        ['Reflected Power', `${rf.reflected_power_w.toFixed(1)} W`],
        ['SWR (calculated)', rf.swr != null ? rf.swr.toFixed(2) : '—'],
    ]);

    const v = sig.voltages;
    html += card('PSU Voltages', [
        ['VCC26 (nom. 26.0 V)', `${v.vcc26_v.toFixed(1)} V`, Math.abs(v.vcc26_v - 26.0) > 1.5],
        ['VCC5  (nom. 5.0 V)',  `${v.vcc5_v.toFixed(3)} V`,  Math.abs(v.vcc5_v - 5.0) > 0.3],
        ['HV1   (nom. 50 V)',   `${v.hv1_v.toFixed(1)} V`,   Math.abs(v.hv1_v - 50.0) > 3.0],
        ['HV2', v.hv2_v != null ? `${v.hv2_v.toFixed(1)} V` : 'N/A (PAM2 not fitted)'],
    ]);

    const c = sig.currents;
    const t = sig.temperatures;
    html += card('PAM Currents & Temperatures', [
        ['PAM1 Current',  `${c.pam1_a.toFixed(3)} A`],
        ['PAM2 Current',  c.pam2_a != null ? `${c.pam2_a.toFixed(3)} A` : 'N/A'],
        ['PAM1 Temp',     t.pam1_celsius != null ? `${t.pam1_celsius.toFixed(1)} °C` : 'N/A'],
        ['PAM2 Temp',     t.pam2_celsius != null ? `${t.pam2_celsius.toFixed(1)} °C` : 'N/A'],
        ['PSU1 Temp',     t.psu1_celsius != null ? `${t.psu1_celsius.toFixed(1)} °C` : 'N/A'],
        ['PSU2 Temp',     t.psu2_celsius != null ? `${t.psu2_celsius.toFixed(1)} °C` : 'N/A'],
    ]);

    const pw = sig.power;
    html += card('DC Power', [
        ['PAM1 Input',       `${pw.pam1_dc_w.toFixed(1)} W`],
        ['PAM2 Input',       `${pw.pam2_dc_w.toFixed(1)} W`],
        ['PAM1 Dissipation', `${pw.pam1_dis_w.toFixed(1)} W`],
        ['PAM2 Dissipation', `${pw.pam2_dis_w.toFixed(1)} W`],
    ]);

    const bm = sig.bias.measured, bn = sig.bias.nominal;
    html += card('Bias Voltages (Measured / Nominal)', [
        ['bias_1a', `${bm.v_1a.toFixed(3)} V  /  ${bn.v_1a.toFixed(3)} V`],
        ['bias_1b', `${bm.v_1b.toFixed(3)} V  /  ${bn.v_1b.toFixed(3)} V`],
        ['bias_1c', `${bm.v_1c.toFixed(3)} V  /  ${bn.v_1c.toFixed(3)} V`],
        ['bias_1d', `${bm.v_1d.toFixed(3)} V  /  ${bn.v_1d.toFixed(3)} V`],
        ['bias_2a', `${bm.v_2a.toFixed(3)} V  /  ${bn.v_2a.toFixed(3)} V`],
        ['bias_2b', `${bm.v_2b.toFixed(3)} V  /  ${bn.v_2b.toFixed(3)} V`],
        ['bias_2c', `${bm.v_2c.toFixed(3)} V  /  ${bn.v_2c.toFixed(3)} V`],
        ['bias_2d', `${bm.v_2d.toFixed(3)} V  /  ${bn.v_2d.toFixed(3)} V`],
    ]);

    const cat = sig.cat;
    html += card('CAT Interface', [
        ['Settings Raw',   `0x${cat.settings_raw.toString(16).toUpperCase().padStart(4,'0')}`],
        ['Command Set',    catCommandSet(cat.command_set)],
        ['Baud Rate',      catBaud(cat.baud_rate_code)],
        ['Byte Spacing',   `${cat.byte_spacing_us} µs`],
        ['Poll Interval',  `${cat.poll_interval_ms} ms`],
    ]);

    html += card('Diagnostics', [
        ['Error Source', `0x${sig.error_source.toString(16).toUpperCase().padStart(4,'0')}`],
        ['LPF Register', `0x${sig.lpf_status.raw.toString(16).toUpperCase().padStart(4,'0')}`],
        ['Band Data',    `${sig.band_data.millivolts} mV`],
    ]);

    html += flagCard('User Flags', sig.user_flags, {
        'EXTRA_COOLING':        ['Extra Cooling',     false],
        'AUTO_OPERATE':         ['Auto Operate',      false],
        'TEMP_UNIT_FAHRENHEIT': ['Temp: Fahrenheit',  false],
        'BUZZER_ENABLED':       ['Buzzer',            false],
        'ATAC_TYPE_AUTO':       ['ATAC: Auto',        false],
        'OPERATE_UNLOCKED':     ['Operate Unlocked',  false],
        'SHUTDOWN_HARD_FAULT':  ['Last Shutdown: HF', true ],
    });

    html += flagCard('Amp Flags 1', sig.amp_flags1, {
        'TURN_ON_PWR_BTN':   ['ON: Front Button',  false],
        'TURN_ON_REMOTE':    ['ON: Remote',        false],
        'TURN_ON_RS232':     ['ON: RS232',         false],
        'BIAS_MON_DISABLED': ['Bias Mon Disabled', true ],
        'BIAS_ENABLED':      ['Bias: ON',          false],
        'HV_MON_DISABLED':   ['HV Mon Disabled',   true ],
        'HV_ON':             ['HV: ON',            false],
        'ATAC_IN_PROGRESS':  ['ATAC Active',       false],
        'LAST_CMD_RS232':    ['Last Cmd: RS232',   false],
    });

    html += flagCard('Amp Flags 2', sig.amp_flags2, {
        'KEYIN_MON':            ['KEYIN Mon',       false],
        'KEYIN_TX_REQUEST':     ['KEYIN: TX Req',   false],
        'INPUT_RELAY_CLOSED':   ['Input Relay: ✓',  false],
        'TX_ACCESS':            ['TX Access: ON',   false],
        'OUTPUT_RELAY_CLOSED':  ['Output Relay: ✓', false],
        'ORC_MON':              ['ORC Mon',         false],
        'ORC':                  ['ORC: ✓ Closed',   false],
        'LPF_TUNED':            ['LPF Tuned',       false],
        'ATU_TUNED':            ['ATU Tuned',       false],
        'ASEL_TUNED':           ['ASEL Tuned',      false],
        'HF_ERR_DISABLED':      ['HF Disabled',     true ],
        'SF_ERR_DISABLED':      ['SF Disabled',     true ],
        'WRN_DISABLED':         ['WRN Disabled',    true ],
    });

    $('results').innerHTML = html;
    setStatus(`✓ Decode complete — ${hard.length} hard, ${soft.length} soft`);
    document.querySelector('.split-right').scrollTop = 0;
}

// ── Flag helpers ─────────────────────────────────────────────────────────────
function parseFlagSet(raw) {
    if (typeof raw !== 'string' || raw.trim() === '') return new Set();
    return new Set(raw.split('|').map(s => s.trim()));
}

function flagCard(title, rawFlags, defs) {
    const active = parseFlagSet(rawFlags);
    const activeNames = [...active].join(' | ') || 'none';
    let html = `<div class="param-card" style="margin-bottom:10px;">`;
    html += `<h4>${title}</h4>`;
    html += `<div style="font-size:10px; color:#6B6B6B; margin-bottom:6px; font-family:monospace;">${activeNames}</div>`;
    html += `<div class="flag-list">`;
    Object.entries(defs).forEach(([constName, [label, isDanger]]) => {
        const isActive = active.has(constName);
        const cls = isActive ? (isDanger ? 'flag-pill active danger' : 'flag-pill active') : 'flag-pill';
        html += `<span class="${cls}">${label}</span>`;
    });
    html += `</div></div>`;
    return html;
}

function catCommandSet(code) {
    return ['Unknown', 'ICOM / Compatibles', 'Yaesu', 'Kenwood', 'Elecraft'][code] ?? `Unknown (code ${code})`;
}

function catBaud(code) {
    return [null, 1200, 4800, 9600, 19200][code] != null ? `${[null,1200,4800,9600,19200][code]} bps` : `Unknown (code ${code})`;
}

// ============================================================================
// LEGACY (1000/1500/2100) Logic
// ============================================================================
const LEGACY_SAMPLES = {
    '1000': ['3asb26','000000','000008','3baa94','0320dc','d77fe1'],
    '1500': ['1atr6b','000000','009000','1BA29D','F723DE','D47FC4'],
    '2100': ['1asb24','000000','000000','000000','000000','000000'],
};

async function decodeLegacy() {
    const model = $('legacyModel').value;
    const groups = [1,2,3,4,5,6].map(i => $(`lg${i}`).value.trim());
    if (groups.some(g => !g)) { setStatus('⚠ All 6 columns required'); return; }
    
    setStatus('Decoding…');
    try {
        const response = await invoke('decode_legacy', { model, groups });
        renderLegacyResults(response.signature, response.diagnosis);
        setStatus(`✓ ACOM ${model} decode complete`);
    } catch (err) {
        $('results').innerHTML = `<div class="fault-hard" style="padding:20px">⚠ Decode error: ${err}</div>`;
        setStatus('⚠ Decode failed: ' + err);
    }
}

function renderLegacyResults(sig, diagnosis) {
    let html = '';
    const modelName = { Acom1000:'ACOM 1000', Acom1500:'ACOM 1500', Acom2100:'ACOM 2100' }[sig.model] ?? sig.model;

    html += `<div class="decode-header">═══════════════════════════════════════════════════════════════\n`;
    html += `  ${modelName} HARD FAULT SIGNATURE DECODE  —  ${new Date().toLocaleString()}\n`;
    html += `═══════════════════════════════════════════════════════════════</div>`;

    // 1. Substitutions
    const unconfirmed = sig.substitutions.filter(s => !s.confirmed);
    if (unconfirmed.length > 0) {
        html += `<div class="subst-warning"><strong>⚠ Verify Display:</strong> Character '${unconfirmed[0].original}' assumed as '${unconfirmed[0].assumed}'.</div>`;
    }

    // 2. State
    html += card('Amplifier State', [
        ['Trip Number',   sig.state.trip_number],
        ['Operating State', sig.state.state_description],
        ['Mode',          sig.state.mode.description],
        ['Fault Signal',  `${sig.state.fault_signal.signal_name} — ${sig.state.fault_signal.description}`],
        ['Signal Type',   sig.state.fault_signal.signal_type],
    ]);

    // 3. Analog Parameters (Using scaled values from Rust)
    // ── Diagnostic analysis ───────────────────────────────────────────────────
    if (diagnosis && diagnosis.findings && diagnosis.findings.length > 0) {
        html += renderLegacyDiagnosis(diagnosis);
    }

    const a = sig.analog;
    html += `<div class="param-card"><h4>Analog Parameters</h4>`;
    html += legacyParam('HV Plate Voltage',   `${a.hvm_v} V`,              a.hvm_raw);
    html += legacyParam('Plate Current',      `${a.ipm_ma} mA`,            a.ipm_raw);
    html += legacyParam('Forward Power',      `${a.pfwd_w.toFixed(1)} W`,  a.pfwd_raw);
    html += legacyParam('Reflected Power',    `${a.rfl_w.toFixed(1)} W`,   a.rfl_raw);
    html += legacyParam('Input Drive',        `${a.inp_w < 0.1 && a.inp_raw > 0 ? a.inp_w.toFixed(3) : a.inp_w.toFixed(1)} W`, a.inp_raw);
    html += legacyParam('PA Anode Avg',       `${a.paav_v.toFixed(0)} V`,  a.paav_raw);
    html += legacyParam('Screen Grid',        `${a.g2c_ma.toFixed(1)} mA`, a.g2c_raw);
    html += legacyParam('Temperature',        `${a.temp_c} °C`,            a.temp_raw);
    html += `</div>`;

    // 4. Digital Signals
    html += `<div class="signal-grid">`;
    html += renderReg('BUFFER 0', sig.registers.buffer0);
    html += renderReg('BUFFER 1', sig.registers.buffer1);
    html += renderReg('PORT 1', sig.registers.port1);
    html += renderReg('PORT 4', sig.registers.port4);
    html += `</div>`;

    // 5. Checksum (confirmed algorithm)
    const csCls = sig.checksum_ok ? 'fault-ok' : 'fault-hard';
    const csText = sig.checksum_ok ? '✓ Valid' : '✗ Mismatch — check for transcription errors';
    html += `<div style="font-size:12px; margin-top:10px;">Checksum: <span class="${csCls}">${csText}</span></div>`;

    $('results').innerHTML = html;
}

// ── Legacy Helpers ───────────────────────────────────────────────────────────
function legacyParam(label, val, raw) {
    let rawHex = "";
    if (Array.isArray(raw)) {
        rawHex = raw.map(b => b.toString(16).padStart(2,'0')).join(' ').toUpperCase();
    } else {
        rawHex = raw.toString(16).padStart(2,'0').toUpperCase();
    }
    return `<div class="param-line"><span>${label}</span><span class="pval">${val} <small style="color:#666;">[0x${rawHex}]</small></span></div>`;
}

function renderReg(name, signals) {
    let html = `<div class="signal-register"><h5>${name}</h5>`;
    signals.forEach(s => {
        if (!s.meaningful) return;
        const dotCls = s.active ? (s.active_low ? 'signal-dot active-danger' : 'signal-dot active') : 'signal-dot';
        html += `<div class="signal-bit"><span class="signal-name ${s.active ? 'active' : ''}">${s.name}</span><div class="${dotCls}"></div></div>`;
    });
    return html + `</div>`;
}

// ============================================================================
// Common UI Helpers
// ============================================================================
function card(title, rows) {
    let html = `<div class="param-card"><h4>${title}</h4>`;
    rows.forEach(([label, value, alert]) => {
        html += `<div class="param-line"><span>${label}</span><span class="pval ${alert ? 'alert' : ''}">${value}</span></div>`;
    });
    return html + `</div>`;
}

function decodeAmpMode(mode) {
    if (typeof mode === 'string') return mode;
    if (typeof mode === 'object' && mode !== null) {
        const key = Object.keys(mode)[0];
        return `${key} (0x${mode[key].toString(16).toUpperCase()})`;
    }
    return String(mode);
}

function renderDiagnosis(diagnosis) {
    if (!diagnosis || !diagnosis.findings || diagnosis.findings.length === 0) return '';
    let html = `<hr class="decode-separator"><div class="section-label">DIAGNOSTIC ANALYSIS</div>`;
    diagnosis.findings.forEach(f => {
        html += `<div class="diag-finding"><strong>${f.title}</strong>: ${f.explanation}<br><em>Action: ${f.action}</em></div>`;
    });
    return html;
}

// ── Input Formatting ─────────────────────────────────────────────────────────
function autoSpaceHex(lineNum) {
    const input = $(`line${lineNum}`);
    const raw = input.value.replace(/\s+/g, '').toUpperCase();
    if (!raw) return;
    const groups = [];
    for (let i = 0; i < raw.length && groups.length < 16; i += 4) groups.push(raw.substring(i, i + 4));
    input.value = groups.join(' ');
    validateLineChecksum(lineNum);
}

function validateLineChecksum(lineNum) {
    const input = $(`line${lineNum}`);
    const ind = $(`check${lineNum}`);
    const words = input.value.trim().split(/\s+/).filter(v => v);
    if (words.length !== 16) { setIndicator(ind, 'invalid'); return; }
    const sum = words.reduce((acc, w) => acc + parseInt(w, 16), 0);
    setIndicator(ind, sum % 65536 === 0 ? 'valid' : 'invalid');
}

function setIndicator(el, state) {
    el.className = 'checksum-indicator';
    if (state === 'valid') { el.classList.add('checksum-valid'); el.textContent = '✓'; }
    else if (state === 'invalid') { el.classList.add('checksum-invalid'); el.textContent = '✗'; }
    else { el.classList.add('checksum-empty'); el.textContent = '?'; }
}

// ── Tab & Navigation ─────────────────────────────────────────────────────────
function switchTab(tab) {
    $('panel-500s').style.display  = tab === '500s'   ? '' : 'none';
    $('panel-legacy').style.display = tab === 'legacy' ? '' : 'none';
    $('tab-500s').classList.toggle('active', tab === '500s');
    $('tab-legacy').classList.toggle('active', tab === 'legacy');
    $('results').innerHTML = '<div class="welcome-message"><p>Ready.</p></div>';
}

function loadSample() { SAMPLE_LINES.forEach((l, i) => { $(`line${i+1}`).value = l; autoSpaceHex(i+1); }); }
function loadLegacySample() {
    const sample = LEGACY_SAMPLES[$('legacyModel').value] || LEGACY_SAMPLES['1000'];
    sample.forEach((g, i) => { $(`lg${i+1}`).value = g; });
    validateLegacyChecksum();
}
function clearAll() { [1,2,3,4].forEach(i => { $(`line${i}`).value = ''; setIndicator($(`check${i}`), 'empty'); }); $('results').innerHTML = '<div class="welcome-message"><p>Cleared.</p></div>'; setStatus('Ready'); }
function clearLegacy() { [1,2,3,4,5,6].forEach(i => $(`lg${i}`).value = ''); $('results').innerHTML = '<div class="welcome-message"><p>Cleared.</p></div>'; setStatus('Ready'); setLegacyIndicator('empty'); }
function clearHistory() { signatureHistory = []; renderHistory(); }
function setStatus(msg) { $('statusBar').textContent = msg; }
function updateSerialStatus(msg, color) { const el = $('serialStatus'); el.textContent = msg; el.style.color = color; }
function updateCaptureStatus(html) { $('captureStatus').innerHTML = html; }
// ── Raw byte formatter ───────────────────────────────────────────────────────
function fmtRaw(raw) {
    if (raw == null) return '??';
    if (Array.isArray(raw)) return raw.map(b => b.toString(16).padStart(2,'0').toUpperCase()).join(' ');
    return raw.toString(16).padStart(2,'0').toUpperCase();
}

// ── Capture history ───────────────────────────────────────────────────────────
function addToHistory(sig) {
    signatureHistory.unshift(sig);
    renderHistory();
}

function renderHistory(activeIdx = -1) {
    const list = $('historyList');
    if (!list) return;
    if (signatureHistory.length === 0) {
        list.innerHTML = '<div class="history-empty">No signatures captured yet</div>';
        return;
    }
    list.innerHTML = signatureHistory.map((sig, i) => `
        <div class="history-item ${i === activeIdx ? 'active' : ''}" onclick="loadFromHistory(${i})">
            <div class="history-meta">
                <span class="history-id">#${sig.id}</span>
                <span class="history-time">${sig.timestamp.toLocaleTimeString()}</span>
            </div>
            <div class="history-preview">${sig.lines[0].substring(0,28)}…</div>
        </div>
    `).join('');
}

function loadFromHistory(index) {
    const sig = signatureHistory[index];
    if (!sig) return;
    for (let i = 0; i < 4; i++) { $(`line${i+1}`).value = sig.lines[i]; autoSpaceHex(i+1); }
    renderHistory(index);
    setTimeout(() => decodeLines(sig.lines), 100);
    document.querySelector('.split-right').scrollTop = 0;
}

function buildTextCapture(n, lines) {
    const ts = new Date().toLocaleString();
    return `ACOM Fault Signature #${n}\nCaptured: ${ts}\n${'='.repeat(70)}\n\n` +
           lines.map((l,i) => `Line ${i+1}: ${l}`).join('\n') + '\n';
}

// ── Legacy input helpers ──────────────────────────────────────────────────────
function autoUpperLegacy(id) {
    const el = $(id);
    const pos = el.selectionStart;
    el.value = el.value.toLowerCase();  // keep lowercase for consistency
    el.setSelectionRange(pos, pos);
}

function validateLegacyChecksum() {
    const ind = $('legacyChecksumIndicator');
    if (!ind) return;

    const fields = ['lg1','lg2','lg3','lg4','lg5','lg6'].map(id => $(id).value.trim().toLowerCase());

    // Need all groups to be the right length
    if (fields.some(f => f.length !== 6)) {
        setLegacyIndicator('empty');
        return;
    }

    // Parse mode code from group1 chars 2-3
    // Known codes: pn=01, pr=02, sb=03, tr=04. Anything else treated as raw hex byte.
    const modeCode = fields[0].slice(2, 4);
    const modeMap  = { pn: 0x01, pr: 0x02, sb: 0x03, tr: 0x04 };
    const modeNum  = modeMap[modeCode] ?? (parseInt(modeCode, 16) || 0);

    // Normalise 7-seg chars to hex
    function norm(ch) {
        if ('0123456789abcdef'.includes(ch)) return ch;
        if (ch === 's') return '5';
        if (ch === 'r') return '5';
        if (ch === 't') return '4';
        return null;
    }

    function parseHexByte(s) {
        const n0 = norm(s[0]), n1 = norm(s[1]);
        if (!n0 || !n1) return null;
        return parseInt(n0 + n1, 16);
    }

    // Extract bytes in checksum order (matches Rust implementation)
    const g1 = fields[0], g2 = fields[1], g3 = fields[2];
    const g4 = fields[3], g5 = fields[4], g6 = fields[5];

    const secondary = parseHexByte(g1.slice(4, 6));
    const pfwd      = parseHexByte(g2.slice(0, 2));
    const rfl       = parseHexByte(g2.slice(2, 4));
    const inp       = parseHexByte(g2.slice(4, 6));
    const paav      = parseHexByte(g3.slice(0, 2));
    const g2c       = parseHexByte(g3.slice(2, 4));
    const ipm       = parseHexByte(g3.slice(4, 6));
    // g4[0..2] = GAMA display byte — NOT included in checksum
    const hvm       = parseHexByte(g4.slice(2, 4));
    const temp      = parseHexByte(g4.slice(4, 6));
    const buf0      = parseHexByte(g5.slice(0, 2));
    const buf1      = parseHexByte(g5.slice(2, 4));
    const port1     = parseHexByte(g5.slice(4, 6));
    const port3     = parseHexByte(g6.slice(0, 2));
    const port4     = parseHexByte(g6.slice(2, 4));
    const csGiven   = parseHexByte(g6.slice(4, 6));

    const bytes = [secondary, pfwd, rfl, inp, paav, g2c, ipm, hvm, temp,
                   buf0, buf1, port1, port3, port4];

    if (bytes.some(b => b === null) || csGiven === null) {
        setLegacyIndicator('empty');
        return;
    }

    let cs = 0xA5 ^ modeNum;
    for (const b of bytes) cs ^= b;

    setLegacyIndicator(cs === csGiven ? 'valid' : 'invalid');
}

function setLegacyIndicator(state) {
    const ind = $('legacyChecksumIndicator');
    if (!ind) return;
    ind.className = 'legacy-checksum-indicator';
    if (state === 'valid') {
        ind.classList.add('cs-valid');
        ind.innerHTML = '✓ Checksum valid';
    } else if (state === 'invalid') {
        ind.classList.add('cs-invalid');
        ind.innerHTML = '✗ Checksum mismatch — check for transcription errors';
    } else {
        ind.classList.add('cs-empty');
        ind.innerHTML = '? Enter all groups to verify';
    }
}

// ── Legacy diagnostic renderer ────────────────────────────────────────────────
function renderLegacyDiagnosis(diagnosis) {
    const sevConfig = {
        'critical': { cls: 'diag-critical', badge: 'diag-critical-badge', icon: '⛔', label: 'CRITICAL' },
        'warning':  { cls: 'diag-warning',  badge: 'diag-warning-badge',  icon: '⚠',  label: 'WARNING'  },
        'info':     { cls: 'diag-info',     badge: 'diag-info-badge',     icon: 'ℹ',  label: 'INFO'     },
    };

    let html = `<hr class="decode-separator">`;
    html += `<div class="section-label">DIAGNOSTIC ANALYSIS</div>`;
    html += `<div class="diag-summary diag-critical-bg" style="margin-bottom:12px;">${diagnosis.summary}</div>`;

    diagnosis.findings.forEach(f => {
        const cfg = sevConfig[f.severity] ?? sevConfig['info'];
        html += `<div class="diag-finding ${cfg.cls}">
            <div class="diag-finding-header">
                <span class="diag-badge ${cfg.badge}">${cfg.icon} ${cfg.label}</span>
                <span class="diag-title">${f.title}</span>
            </div>
            <div class="diag-explanation">${f.explanation}</div>
            <div class="diag-action"><strong>→ Action:</strong> ${f.action}</div>
        </div>`;
    });

    return html;
}

// ── Load signature from file ──────────────────────────────────────────────────
async function loadSignatureFile() {
    try {
        const { open } = window.__TAURI__.dialog;
        const sigDir = await getSignaturesDir();
        const selected = await open({
            multiple: false,
            filters: [{ name: 'ACOM Signature', extensions: ['txt'] }],
            defaultPath: sigDir || undefined,
        });
        if (!selected) return;

        const content = await invoke('read_signature_file', { path: selected });

        // Parse: lines starting with "Line N:" or just raw hex lines
        const lines = content.split('\n')
            .map(l => l.replace(/^Line \d+:\s*/i, '').trim())
            .filter(l => /^[0-9A-Fa-f\s]+$/.test(l) && l.replace(/\s/g,'').length >= 60);

        if (lines.length < 4) {
            setStatus('⚠ Could not parse signature file — expected 4 hex lines');
            return;
        }

        for (let i = 0; i < 4; i++) {
            $(`line${i+1}`).value = lines[i];
            autoSpaceHex(i+1);
        }
        setStatus('✓ Signature loaded from file');
        await decodeLines([1,2,3,4].map(i => $(`line${i}`).value.trim()));
    } catch (err) {
        setStatus('⚠ Failed to load file: ' + err);
    }
}

async function getSignaturesDir() {
    try {
        const base = await invoke('get_signatures_dir');
        return base;
    } catch { return null; }
}

// ── Open signatures folder ────────────────────────────────────────────────────
async function openSignaturesFolder() {
    try {
        await invoke('open_signatures_folder');
    } catch (err) {
        setStatus('⚠ Could not open folder: ' + err);
    }
}
