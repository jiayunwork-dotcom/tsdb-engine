<template>
  <div>
    <div class="page-header">
      <div class="flex items-center justify-between" style="width:100%">
        <div>
          <h1>Alert Events</h1>
          <p>Real-time alert event timeline with WebSocket updates</p>
        </div>
        <div class="flex gap-2 items-center">
          <span v-if="wsConnected" class="ws-indicator ws-connected" title="WebSocket connected">Live</span>
          <span v-else class="ws-indicator ws-disconnected" title="WebSocket disconnected">Offline</span>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Filters</h3>
      </div>
      <div class="flex gap-2 flex-wrap" style="align-items: flex-end;">
        <div class="form-group" style="width: 140px;">
          <label>Severity</label>
          <select v-model="filters.severity" @change="loadEvents">
            <option value="">All</option>
            <option value="critical">Critical</option>
            <option value="warning">Warning</option>
            <option value="info">Info</option>
          </select>
        </div>
        <div class="form-group" style="width: 160px;">
          <label>Rule Name</label>
          <input v-model="filters.rule_name" placeholder="Filter by name" @change="loadEvents" />
        </div>
        <div class="form-group" style="width: 180px;">
          <label>Start Time</label>
          <input type="datetime-local" v-model="filters.startTime" @change="loadEvents" />
        </div>
        <div class="form-group" style="width: 180px;">
          <label>End Time</label>
          <input type="datetime-local" v-model="filters.endTime" @change="loadEvents" />
        </div>
        <button class="btn btn-secondary" @click="resetFilters">Reset</button>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Events ({{ total }})</h3>
      </div>
      <div v-if="events.length === 0" style="color: var(--text-muted); font-size: 13px; padding: 8px 0;">No alert events found</div>
      <div v-else class="timeline">
        <div
          v-for="event in events"
          :key="event.id"
          class="timeline-item"
          :class="{ 'new-event': event._isNew }"
        >
          <div class="timeline-dot" :class="dotClass(event)"></div>
          <div class="timeline-content">
            <div class="timeline-header">
              <span class="badge" :class="severityClass(event.severity)">{{ event.severity }}</span>
              <span class="badge" :class="typeClass(event.event_type)">{{ event.event_type }}</span>
              <span class="timeline-rule">{{ event.rule_name }}</span>
              <span class="timeline-time">{{ formatTime(event.timestamp) }}</span>
            </div>
            <div class="timeline-body">
              <div class="timeline-metric">
                <span class="label">Metric:</span>
                <span style="font-family:var(--mono);font-size:12px">{{ event.metric }}</span>
              </div>
              <div class="timeline-value">
                <span class="label">Value:</span>
                <span class="value-num" :class="event.event_type === 'firing' ? 'value-alert' : 'value-ok'">
                  {{ event.value?.toFixed(4) }}
                </span>
                <span class="label" style="margin-left:8px">Threshold:</span>
                <span>{{ event.threshold }}</span>
              </div>
              <div v-if="event.tags && Object.keys(event.tags).length > 0" class="timeline-tags">
                <span v-for="(v, k) in event.tags" :key="k" class="tag-chip">{{ k }}={{ v }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div v-if="events.length < total" class="pagination">
        <button class="btn btn-secondary" @click="loadMore">Load More ({{ total - events.length }} remaining)</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import { getAlertEvents, createAlertWs } from '../api'

const events = ref([])
const total = ref(0)
const wsConnected = ref(false)
let ws = null
let wsReconnectTimer = null

const filters = ref({
  severity: '',
  rule_name: '',
  startTime: '',
  endTime: '',
})

const currentOffset = ref(0)
const pageSize = 50

function formatTime(tsNanos) {
  const ms = tsNanos / 1_000_000
  return new Date(ms).toLocaleString()
}

function severityClass(sev) {
  if (sev === 'critical') return 'badge-danger'
  if (sev === 'warning') return 'badge-warning'
  return 'badge-info'
}

function typeClass(type) {
  if (type === 'firing') return 'badge-danger'
  if (type === 'resolved') return 'badge-success'
  return 'badge-info'
}

function dotClass(event) {
  if (event.event_type === 'firing') return 'dot-firing'
  if (event.event_type === 'resolved') return 'dot-resolved'
  return 'dot-default'
}

function buildParams(offset = 0) {
  const params = { offset, limit: pageSize }
  if (filters.value.severity) params.severity = filters.value.severity
  if (filters.value.rule_name) params.rule_name = filters.value.rule_name
  if (filters.value.startTime) {
    params.start_time = new Date(filters.value.startTime).getTime() * 1_000_000
  }
  if (filters.value.endTime) {
    params.end_time = new Date(filters.value.endTime).getTime() * 1_000_000
  }
  return params
}

async function loadEvents() {
  try {
    const params = buildParams(0)
    const data = await getAlertEvents(params)
    events.value = data.events || []
    total.value = data.total || 0
    currentOffset.value = events.value.length
  } catch (e) {
    console.error('Failed to load events:', e)
  }
}

async function loadMore() {
  try {
    const params = buildParams(currentOffset.value)
    const data = await getAlertEvents(params)
    events.value = [...events.value, ...(data.events || [])]
    total.value = data.total || 0
    currentOffset.value = events.value.length
  } catch (e) {
    console.error('Failed to load more events:', e)
  }
}

function resetFilters() {
  filters.value = { severity: '', rule_name: '', startTime: '', endTime: '' }
  loadEvents()
}

function handleWsMessage(event) {
  try {
    const alertEvent = JSON.parse(event.data)
    alertEvent._isNew = true
    events.value.unshift(alertEvent)
    total.value += 1

    if (filters.value.severity && alertEvent.severity !== filters.value.severity) {
      events.value.shift()
      total.value -= 1
    }

    setTimeout(() => {
      const idx = events.value.findIndex(e => e.id === alertEvent.id)
      if (idx !== -1) {
        events.value[idx] = { ...events.value[idx], _isNew: false }
      }
    }, 3000)
  } catch (e) {
    console.error('Failed to parse WS message:', e)
  }
}

function connectWs() {
  if (ws) {
    ws.close()
  }

  ws = createAlertWs()
  ws.onopen = () => { wsConnected.value = true }
  ws.onclose = () => {
    wsConnected.value = false
    wsReconnectTimer = setTimeout(connectWs, 5000)
  }
  ws.onerror = () => { wsConnected.value = false }
  ws.onmessage = handleWsMessage
}

onMounted(() => {
  loadEvents()
  connectWs()
})

onUnmounted(() => {
  if (ws) ws.close()
  if (wsReconnectTimer) clearTimeout(wsReconnectTimer)
})
</script>

<style scoped>
.ws-indicator {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 600;
  gap: 4px;
}

.ws-indicator::before {
  content: "";
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.ws-connected {
  background: rgba(16, 185, 129, 0.15);
  color: var(--success);
}

.ws-connected::before {
  background: var(--success);
  animation: pulse 2s infinite;
}

.ws-disconnected {
  background: rgba(239, 68, 68, 0.15);
  color: var(--danger);
}

.ws-disconnected::before {
  background: var(--danger);
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

.timeline {
  position: relative;
  padding-left: 24px;
}

.timeline::before {
  content: "";
  position: absolute;
  left: 7px;
  top: 0;
  bottom: 0;
  width: 2px;
  background: var(--border);
}

.timeline-item {
  position: relative;
  padding-bottom: 20px;
  transition: background 0.5s ease;
}

.timeline-item.new-event {
  background: rgba(239, 68, 68, 0.06);
  border-radius: 8px;
  margin: -4px -12px;
  padding: 4px 12px;
}

.timeline-dot {
  position: absolute;
  left: -20px;
  top: 6px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  border: 2px solid var(--bg-card);
}

.dot-firing { background: var(--danger); }
.dot-resolved { background: var(--success); }
.dot-default { background: var(--text-muted); }

.timeline-content {
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 12px 16px;
}

.timeline-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  flex-wrap: wrap;
}

.timeline-rule {
  font-weight: 600;
  color: var(--text-h);
  font-size: 13px;
}

.timeline-time {
  margin-left: auto;
  font-size: 12px;
  color: var(--text-muted);
  font-family: var(--mono);
}

.timeline-body {
  font-size: 13px;
  color: var(--text);
}

.timeline-body .label {
  color: var(--text-muted);
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  margin-right: 4px;
}

.timeline-metric, .timeline-value {
  margin-bottom: 4px;
}

.value-alert { color: var(--danger); font-weight: 600; font-family: var(--mono); }
.value-ok { color: var(--success); font-weight: 600; font-family: var(--mono); }

.timeline-tags {
  margin-top: 6px;
}

.timeline-tags .tag-chip {
  font-size: 11px;
  padding: 2px 8px;
}

.pagination {
  display: flex;
  justify-content: center;
  padding-top: 16px;
}
</style>
