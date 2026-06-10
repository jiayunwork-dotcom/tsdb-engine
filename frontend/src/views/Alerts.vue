<template>
  <div>
    <div class="page-header">
      <div class="flex items-center justify-between" style="width:100%">
        <div>
          <h1>Alert Rules</h1>
          <p>Manage alerting rules based on time-series data conditions</p>
        </div>
        <button class="btn btn-primary" @click="showCreateModal = true">+ New Rule</button>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Rules ({{ rules.length }})</h3>
      </div>
      <div v-if="rules.length === 0" style="color: var(--text-muted); font-size: 13px; padding: 8px 0;">No alert rules configured</div>
      <table v-else>
        <thead>
          <tr>
            <th>Name</th>
            <th>Metric</th>
            <th>Condition</th>
            <th>Severity</th>
            <th>State</th>
            <th>Enabled</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in rules" :key="r.id">
            <td style="font-weight:600;color:var(--text-h)">{{ r.name }}</td>
            <td><span style="font-family:var(--mono);font-size:12px">{{ r.metric }}</span></td>
            <td>
              <span style="font-family:var(--mono);font-size:12px">
                {{ r.aggregation }}({{ formatWindow(r.window_secs) }}) {{ r.operator }} {{ r.threshold }}
              </span>
            </td>
            <td>
              <span class="badge" :class="severityClass(r.severity)">{{ r.severity }}</span>
            </td>
            <td>
              <span class="badge" :class="stateClass(r.state)">{{ r.state }}</span>
            </td>
            <td>
              <label class="toggle-switch">
                <input type="checkbox" :checked="r.enabled" @change="toggleRule(r)" />
                <span class="toggle-slider"></span>
              </label>
            </td>
            <td>
              <div class="flex gap-2">
                <button class="btn btn-secondary btn-sm" @click="editRule(r)">Edit</button>
                <button class="btn btn-danger btn-sm" @click="removeRule(r.id)">Delete</button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div v-if="showCreateModal || showEditModal" class="modal-overlay" @click.self="closeModal">
      <div class="modal">
        <div class="modal-header">
          <h3>{{ showEditModal ? 'Edit Rule' : 'New Alert Rule' }}</h3>
          <button class="modal-close" @click="closeModal">&times;</button>
        </div>
        <div class="modal-body">
          <div class="form-grid">
            <div class="form-group">
              <label>Rule Name</label>
              <input v-model="form.name" placeholder="e.g., High CPU Alert" />
            </div>
            <div class="form-group">
              <label>Metric</label>
              <input v-model="form.metric" placeholder="e.g., cpu" />
            </div>
            <div class="form-group">
              <label>Aggregation</label>
              <select v-model="form.aggregation">
                <option value="avg">avg</option>
                <option value="max">max</option>
                <option value="min">min</option>
                <option value="sum">sum</option>
                <option value="count">count</option>
              </select>
            </div>
            <div class="form-group">
              <label>Window (seconds)</label>
              <input v-model.number="form.window_secs" type="number" min="15" />
            </div>
            <div class="form-group">
              <label>Operator</label>
              <select v-model="form.operator">
                <option value=">">&gt; Greater Than</option>
                <option value=">=">&ge; Greater or Equal</option>
                <option value="<">&lt; Less Than</option>
                <option value="<=">&le; Less or Equal</option>
                <option value="==">== Equal</option>
                <option value="!=">!= Not Equal</option>
              </select>
            </div>
            <div class="form-group">
              <label>Threshold</label>
              <input v-model.number="form.threshold" type="number" step="0.1" />
            </div>
            <div class="form-group">
              <label>Trigger Count</label>
              <input v-model.number="form.trigger_count" type="number" min="1" />
            </div>
            <div class="form-group">
              <label>Severity</label>
              <select v-model="form.severity">
                <option value="critical">critical</option>
                <option value="warning">warning</option>
                <option value="info">info</option>
              </select>
            </div>
            <div class="form-group">
              <label>Silence (seconds)</label>
              <input v-model.number="form.silence_secs" type="number" min="0" />
            </div>
            <div class="form-group">
              <label>Tags Filter (key=value, comma separated)</label>
              <input v-model="form.tagsStr" placeholder="e.g., host=server01,region=us-west" />
            </div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="closeModal">Cancel</button>
          <button class="btn btn-primary" @click="saveRule">
            {{ showEditModal ? 'Update' : 'Create' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import {
  getAlertRules, createAlertRule, updateAlertRule,
  deleteAlertRule, enableAlertRule, disableAlertRule
} from '../api'

const rules = ref([])
const showCreateModal = ref(false)
const showEditModal = ref(false)
const editingId = ref(null)

const defaultForm = () => ({
  name: '',
  metric: '',
  aggregation: 'avg',
  window_secs: 300,
  operator: '>',
  threshold: 0,
  trigger_count: 1,
  severity: 'warning',
  silence_secs: 300,
  tagsStr: '',
})

const form = ref(defaultForm())

function formatWindow(secs) {
  if (secs >= 3600) return `${(secs / 3600).toFixed(1)}h`
  if (secs >= 60) return `${(secs / 60).toFixed(0)}m`
  return `${secs}s`
}

function severityClass(sev) {
  if (sev === 'critical') return 'badge-danger'
  if (sev === 'warning') return 'badge-warning'
  return 'badge-info'
}

function stateClass(state) {
  if (state === 'firing') return 'badge-danger'
  if (state === 'pending') return 'badge-warning'
  if (state === 'resolved') return 'badge-success'
  return 'badge-info'
}

function parseTags(str) {
  const tags = {}
  if (!str) return tags
  str.split(',').forEach(pair => {
    const [k, v] = pair.split('=').map(s => s.trim())
    if (k && v) tags[k] = v
  })
  return tags
}

function tagsToString(tags) {
  return Object.entries(tags || {}).map(([k, v]) => `${k}=${v}`).join(', ')
}

function editRule(r) {
  editingId.value = r.id
  form.value = {
    name: r.name,
    metric: r.metric,
    aggregation: r.aggregation,
    window_secs: r.window_secs,
    operator: r.operator,
    threshold: r.threshold,
    trigger_count: r.trigger_count,
    severity: r.severity,
    silence_secs: r.silence_secs,
    tagsStr: tagsToString(r.tags),
  }
  showEditModal.value = true
}

function closeModal() {
  showCreateModal.value = false
  showEditModal.value = false
  editingId.value = null
  form.value = defaultForm()
}

async function saveRule() {
  const payload = {
    name: form.value.name,
    metric: form.value.metric,
    aggregation: form.value.aggregation,
    window_secs: form.value.window_secs,
    operator: form.value.operator,
    threshold: form.value.threshold,
    trigger_count: form.value.trigger_count,
    severity: form.value.severity,
    silence_secs: form.value.silence_secs,
    tags: parseTags(form.value.tagsStr),
    enabled: true,
  }

  try {
    if (showEditModal.value) {
      await updateAlertRule(editingId.value, payload)
    } else {
      await createAlertRule(payload)
    }
    closeModal()
    await loadRules()
  } catch (e) {
    console.error('Failed to save rule:', e)
  }
}

async function removeRule(id) {
  if (!confirm('Delete this alert rule?')) return
  try {
    await deleteAlertRule(id)
    await loadRules()
  } catch (e) {
    console.error('Failed to delete rule:', e)
  }
}

async function toggleRule(r) {
  try {
    if (r.enabled) {
      await disableAlertRule(r.id)
    } else {
      await enableAlertRule(r.id)
    }
    await loadRules()
  } catch (e) {
    console.error('Failed to toggle rule:', e)
  }
}

async function loadRules() {
  try {
    rules.value = await getAlertRules()
  } catch (e) {
    console.error('Failed to load rules:', e)
  }
}

onMounted(loadRules)
</script>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 16px;
  width: 640px;
  max-height: 90vh;
  overflow-y: auto;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px 24px;
  border-bottom: 1px solid var(--border);
}

.modal-header h3 {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-h);
}

.modal-close {
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 24px;
  cursor: pointer;
  padding: 0;
  line-height: 1;
}

.modal-close:hover {
  color: var(--text);
}

.modal-body {
  padding: 20px 24px;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 16px 24px;
  border-top: 1px solid var(--border);
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.form-grid .form-group {
  margin-bottom: 0;
}

.toggle-switch {
  position: relative;
  display: inline-block;
  width: 36px;
  height: 20px;
}

.toggle-switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.toggle-slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: var(--border);
  border-radius: 20px;
  transition: 0.3s;
}

.toggle-slider::before {
  position: absolute;
  content: "";
  height: 14px;
  width: 14px;
  left: 3px;
  bottom: 3px;
  background-color: var(--text-muted);
  border-radius: 50%;
  transition: 0.3s;
}

.toggle-switch input:checked + .toggle-slider {
  background-color: var(--accent);
}

.toggle-switch input:checked + .toggle-slider::before {
  transform: translateX(16px);
  background-color: white;
}
</style>
