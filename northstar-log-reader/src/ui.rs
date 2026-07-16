#![forbid(unsafe_op_in_unsafe_fn)]

pub fn render_live_html(url: &str) -> String {
    let url_json = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_owned());
    format!(r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>North Star LIVE LogReader</title>
<style>
:root {{
  color-scheme: dark;
  font-family: Segoe UI, system-ui, sans-serif;
  background: #090d13;
  color: #d8e2f0;
}}
* {{ box-sizing: border-box; }}
html, body {{ height: 100%; }}
body {{
  margin: 0;
  overflow: hidden;
  display: grid;
  grid-template-rows: auto 1fr auto;
  background: radial-gradient(circle at top left, #132033 0, #090d13 42%);
}}
.top-bar {{
  display: grid;
  grid-template-columns: auto 1fr auto auto;
  gap: 10px;
  align-items: center;
  padding: 12px 14px;
  border-bottom: 1px solid #243247;
  background: rgba(13, 19, 29, 0.94);
}}
.brand {{ font-weight: 700; letter-spacing: 0.02em; color: #f0f6ff; }}
.url-input,
.search-input,
.level-select,
button {{
  height: 36px;
  border-radius: 8px;
  border: 1px solid #314159;
  background: #07101b;
  color: #d8e2f0;
  outline: none;
}}
.url-input,
.search-input {{ padding: 0 12px; width: 100%; }}
.url-input:focus,
.search-input:focus,
.level-select:focus {{ border-color: #6aa9ff; box-shadow: 0 0 0 2px rgba(106,169,255,.12); }}
button {{
  padding: 0 14px;
  background: #1b2a3f;
  cursor: pointer;
}}
button:hover {{ background: #263a56; }}
button.danger {{ background: #3a1d25; border-color: #6b3140; }}
button.danger:hover {{ background: #552937; }}
.log-field {{
  min-height: 0;
  overflow: auto;
  padding: 12px 14px;
}}
.log-list {{
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 100%;
}}
.log-empty {{
  height: 100%;
  min-height: 240px;
  display: grid;
  place-items: center;
  color: #74849a;
  border: 1px dashed #273449;
  border-radius: 12px;
}}
.log-row {{
  display: grid;
  grid-template-columns: minmax(170px, 220px) 64px minmax(180px, 340px) 1fr;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid #1c293a;
  border-radius: 10px;
  background: rgba(10, 16, 25, 0.86);
  font-size: 13px;
}}
.log-row.hidden {{ display: none; }}
.time {{ color: #8fa2bb; font-family: Consolas, ui-monospace, monospace; }}
.level {{ font-weight: 800; font-family: Consolas, ui-monospace, monospace; }}
.target {{ color: #bad0eb; font-family: Consolas, ui-monospace, monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
.message {{ white-space: pre-wrap; overflow-wrap: anywhere; }}
.level-ERROR {{ color: #ff6b6b; }}
.level-WARN {{ color: #ffd166; }}
.level-INFO {{ color: #8ecae6; }}
.level-DEBUG {{ color: #a7c957; }}
.level-TRACE {{ color: #9d8df1; }}
.bottom-search {{
  display: grid;
  grid-template-columns: 170px 1fr 180px;
  gap: 10px;
  align-items: center;
  padding: 12px 14px;
  border-top: 1px solid #243247;
  background: rgba(13, 19, 29, 0.96);
}}
.status-line {{ color:#9fb2cc; font-size:12px; white-space: nowrap; }}
@media (max-width: 900px) {{
  .top-bar {{ grid-template-columns: 1fr; }}
  .bottom-search {{ grid-template-columns: 1fr; }}
  .log-row {{ grid-template-columns: 1fr; }}
}}
</style>
</head>
<body>
  <header class="top-bar">
    <div class="brand">LIVE LogReader</div>
    <input id="url" class="url-input" value="" aria-label="Live log URL">
    <button id="connect">Connect</button>
    <div id="status" class="status-line">idle</div>
  </header>

  <main id="logField" class="log-field">
    <div id="logList" class="log-list">
      <div id="empty" class="log-empty">Waiting for live log events…</div>
    </div>
  </main>

  <footer class="bottom-search">
    <select id="minLevel" class="level-select" aria-label="Logging level filter">
      <option>TRACE</option>
      <option>DEBUG</option>
      <option selected>INFO</option>
      <option>WARN</option>
      <option>ERROR</option>
    </select>
    <input id="search" class="search-input" placeholder="Search logs by message, target, level, event id…" aria-label="Search logs">
    <button id="clear" class="danger">Clear history logs</button>
  </footer>

<script>
const initialUrl = {url_json};
const urlInput = document.getElementById('url');
const logField = document.getElementById('logField');
const logList = document.getElementById('logList');
const empty = document.getElementById('empty');
const statusEl = document.getElementById('status');
const minLevel = document.getElementById('minLevel');
const search = document.getElementById('search');
const connectButton = document.getElementById('connect');
const ranks = {{ TRACE:1, DEBUG:2, INFO:3, WARN:4, ERROR:5 }};
let aborter = null;
let spinnerTimer = null;
let spinnerIndex = 0;
const spinnerFrames = ['|', '/', '-', '\\'];
let events = [];
let rowSeq = 0;
urlInput.value = initialUrl;

function normalize(e) {{
  if (e.schema === 'northstar.ulog.event.v1') {{
    return {{
      time: e.timestamp_utc || '-',
      level: e.level || 'INFO',
      target: (e.source && e.source.name) || '-',
      eventId: e.event_id || '-',
      message: e.message || ''
    }};
  }}
  return {{
    time: e.timestamp_utc || e.timestamp || (e.timestamp_ns ? e.timestamp_ns + 'ns' : '-'),
    level: e.level || 'INFO',
    target: e.target || e.source || '-',
    eventId: e.event_id || '-',
    message: e.message || ''
  }};
}}

function ingestLine(line) {{
  const t = line.trim();
  if (!t || t.startsWith(':') || t.startsWith('event:') || t.startsWith('id:') || t.startsWith('retry:')) return;
  const payload = t.startsWith('data:') ? t.slice(5).trimStart() : t;
  const obj = JSON.parse(payload);
  const event = normalize(obj);
  events.push(event);
  if (events.length > 5000) events.shift();
  appendRow(event);
  applyFilters();
  statusEl.textContent = `LIVE connected · ${{events.length}} events`;
}}

function appendRow(e) {{
  empty.style.display = 'none';
  const row = document.createElement('div');
  row.className = 'log-row';
  row.dataset.level = e.level;
  row.dataset.search = `${{e.time}} ${{e.level}} ${{e.target}} ${{e.eventId}} ${{e.message}}`.toLowerCase();
  row.dataset.seq = String(++rowSeq);
  row.innerHTML = `
    <div class="time">${{escapeHtml(e.time)}}</div>
    <div class="level level-${{escapeHtml(e.level)}}">${{escapeHtml(e.level)}}</div>
    <div class="target" title="${{escapeHtml(e.target)}}">${{escapeHtml(e.target)}}</div>
    <div class="message">${{escapeHtml(e.message)}}</div>`;
  logList.appendChild(row);
  while (logList.children.length > 5001) {{
    const first = logList.children[0];
    if (first && first.id !== 'empty') first.remove(); else break;
  }}
  logField.scrollTop = logField.scrollHeight;
}}

function applyFilters() {{
  const min = ranks[minLevel.value] || 1;
  const q = search.value.trim().toLowerCase();
  let visible = 0;
  for (const row of logList.querySelectorAll('.log-row')) {{
    const levelOk = (ranks[row.dataset.level] || 3) >= min;
    const queryOk = !q || row.dataset.search.includes(q);
    const show = levelOk && queryOk;
    row.classList.toggle('hidden', !show);
    if (show) visible++;
  }}
  if (events.length > 0) statusEl.textContent = `${{aborter ? 'LIVE connected' : 'disconnected'}} · ${{visible}}/${{events.length}} visible`;
}}

function escapeHtml(v) {{
  return String(v).replace(/[&<>"']/g, c => ({{'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}}[c]));
}}

function startConnectingSpinner() {{
  stopConnectingSpinner(false);
  spinnerIndex = 0;
  statusEl.textContent = 'LIVE connecting |';
  spinnerTimer = setInterval(() => {{
    const frame = spinnerFrames[spinnerIndex % spinnerFrames.length];
    spinnerIndex++;
    statusEl.textContent = 'LIVE connecting ' + frame;
  }}, 120);
}}
function stopConnectingSpinner(clearText = true) {{
  if (spinnerTimer) {{ clearInterval(spinnerTimer); spinnerTimer = null; }}
  if (clearText) statusEl.textContent = 'idle';
}}

async function connect() {{
  if (aborter) {{
    aborter.abort();
    aborter = null;
    connectButton.textContent = 'Connect';
    stopConnectingSpinner(false);
    statusEl.textContent = 'disconnected';
    return;
  }}

  aborter = new AbortController();
  connectButton.textContent = 'Disconnect';
  const url = urlInput.value.trim();
  startConnectingSpinner();

  try {{
    const res = await fetch(url, {{
      cache: 'no-store',
      signal: aborter.signal,
      headers: {{ 'Accept': 'application/x-ndjson, text/event-stream, application/json' }}
    }});
    if (!res.ok) throw new Error(res.status + ' ' + res.statusText);
    stopConnectingSpinner(false);
    statusEl.textContent = 'LIVE connected: ' + url;
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';

    while (true) {{
      const {{ value, done }} = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, {{ stream: true }});
      const parts = buffer.split(/\r?\n/);
      buffer = parts.pop() || '';
      for (const line of parts) ingestLine(line);
    }}
    if (buffer.trim()) ingestLine(buffer);
  }} catch (err) {{
    if (!aborter || err.name === 'AbortError') return;
    statusEl.textContent = 'error: ' + err.message;
  }} finally {{
    aborter = null;
    connectButton.textContent = 'Connect';
    stopConnectingSpinner(false);
  }}
}}

connectButton.onclick = connect;
document.getElementById('clear').onclick = () => {{
  events = [];
  rowSeq = 0;
  logList.innerHTML = '';
  logList.appendChild(empty);
  empty.style.display = 'grid';
  statusEl.textContent = 'history cleared';
}};
minLevel.onchange = applyFilters;
search.oninput = applyFilters;
</script>
</body>
</html>
"#)
}
