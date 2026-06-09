<template>
  <div>
    <div class="page-header">
      <h1>Alert Configuration</h1>
      <p>Set up alerting rules based on query conditions</p>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Create Alert Rule</h3>
      </div>
      <div class="flex gap-2 flex-wrap" style="align-items: flex-end;">
        <div class="form-group" style="flex: 1; min-width: 150px;">
          <label>Metric</label>
          <input v-model="newAlert.metric" placeholder="e.g., cpu" />
        </div>
        <div class="form-group" style="width: 130px;">
          <label>Condition</label>
          <select v-model="newAlert.condition">
            <option value="gt">Greater Than</option>
            <option value="lt">Less Than</option>
            <option value="gte">Greater or Equal</option>
            <option value="lte">Less or Equal</option>
            <option value="eq">Equal</option>
          </select>
        </div>
        <div class="form-group" style="width: 120px;">
          <label>Threshold</label>
          <input v-model.number="newAlert.threshold" type="number" step="0.1" />
        </div>
        <div class="form-group" style="width: 120px;">
          <label>Duration (secs)</label>
          <input v-model.number="newAlert.duration_secs" type="number" min="0" />
        </div>
        <button class="btn btn-primary" @click="createNewAlert">Create Alert</button>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Active Alert Rules</h3>
      </div>
      <div v-if="alerts.length === 0" style="color: var(--text-muted); font-size: 13px; padding: 8px 0;">No alert rules configured</div>
      <table v-else>
        <thead>
          <tr>
            <th>ID</th>
            <th>Metric</th>
            <th>Condition</th>
            <th>Threshold</th>
            <th>Duration</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="a in alerts" :key="a.id">
            <td style="font-family: var(--mono); font-size: 11px;">{{ a.id.substring(0, 8) }}</td>
            <td>{{ a.metric }}</td>
            <td>{{ formatCondition(a.condition) }}</td>
            <td>{{ a.threshold }}</td>
            <td>{{ a.duration_secs }}s</td>
            <td>
              <span class="badge" :class="a.enabled ? 'badge-success' : 'badge-warning'">
                {{ a.enabled ? 'Enabled' : 'Disabled' }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Alert History</h3>
      </div>
      <div v-if="alertHistory.length === 0" style="color: var(--text-muted); font-size: 13px; padding: 8px 0;">No alerts triggered yet</div>
      <table v-else>
        <thead>
          <tr>
            <th>Alert ID</th>
            <th>Metric</th>
            <th>Value</th>
            <th>Threshold</th>
            <th>Time</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(h, i) in alertHistory" :key="i">
            <td style="font-family: var(--mono); font-size: 11px;">{{ h.alert_id }}</td>
            <td>{{ h.metric }}</td>
            <td>{{ h.value?.toFixed(2) }}</td>
            <td>{{ h.threshold }}</td>
            <td>{{ new Date(h.timestamp / 1_000_000).toLocaleString() }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getAlerts, createAlert, getAlertHistory } from '../api'

const alerts = ref([])
const alertHistory = ref([])
const newAlert = ref({
  metric: '',
  condition: 'gt',
  threshold: 0,
  duration_secs: 60,
  enabled: true,
})

function formatCondition(cond) {
  const map = { gt: '>', lt: '<', gte: '>=', lte: '<=', eq: '=' }
  return map[cond] || cond
}

async function createNewAlert() {
  if (!newAlert.value.metric) return
  try {
    const alert = {
      id: crypto.randomUUID(),
      ...newAlert.value,
      tags: {},
    }
    await createAlert(alert)
    alerts.value.push(alert)
    newAlert.value = { metric: '', condition: 'gt', threshold: 0, duration_secs: 60, enabled: true }
  } catch (e) {
    console.error('Failed to create alert:', e)
  }
}

onMounted(async () => {
  try {
    const [a, h] = await Promise.all([getAlerts(), getAlertHistory()])
    alerts.value = a || []
    alertHistory.value = h || []
  } catch (e) {
    console.error('Failed to load alerts:', e)
  }
})
</script>
