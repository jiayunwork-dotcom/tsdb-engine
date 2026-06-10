<template>
  <div>
    <div class="page-header">
      <h1>System Administration</h1>
      <p>Manage retention policies, WAL configuration, and block storage</p>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Retention Policies</h3>
        <button class="btn btn-sm btn-primary" @click="showRetentionForm = !showRetentionForm">+ Add Policy</button>
      </div>

      <div v-if="showRetentionForm" style="margin-bottom: 16px; padding: 16px; background: var(--bg); border-radius: 8px;">
        <div class="flex gap-2 flex-wrap">
          <div class="form-group" style="flex: 1; min-width: 150px;">
            <label>Metric</label>
            <input v-model="newRetention.metric" placeholder="e.g., cpu" />
          </div>
          <div class="form-group" style="width: 100px;">
            <label>TTL (days)</label>
            <input v-model.number="newRetention.ttl_days" type="number" min="1" />
          </div>
          <div class="form-group" style="width: 140px;">
            <label>7d Downsample</label>
            <select v-model="newRetention.downsample_7d_interval_secs">
              <option :value="null">Disabled</option>
              <option :value="60">1 minute</option>
              <option :value="300">5 minutes</option>
              <option :value="3600">1 hour</option>
            </select>
          </div>
          <div class="form-group" style="width: 140px;">
            <label>30d Downsample</label>
            <select v-model="newRetention.downsample_30d_interval_secs">
              <option :value="null">Disabled</option>
              <option :value="3600">1 hour</option>
              <option :value="21600">6 hours</option>
              <option :value="86400">1 day</option>
            </select>
          </div>
          <div style="display: flex; align-items: flex-end; padding-bottom: 12px;">
            <button class="btn btn-sm btn-primary" @click="addRetention">Save</button>
          </div>
        </div>
      </div>

      <table v-if="retentionPolicies.length > 0">
        <thead>
          <tr>
            <th>Metric</th>
            <th>TTL (days)</th>
            <th>7d Downsample</th>
            <th>30d Downsample</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="p in retentionPolicies" :key="p.metric">
            <td><strong>{{ p.metric }}</strong></td>
            <td>{{ p.ttl_days }}</td>
            <td>{{ p.downsample_7d_interval_secs ? formatInterval(p.downsample_7d_interval_secs) : '-' }}</td>
            <td>{{ p.downsample_30d_interval_secs ? formatInterval(p.downsample_30d_interval_secs) : '-' }}</td>
          </tr>
        </tbody>
      </table>
      <div v-else style="color: var(--text-muted); font-size: 13px; padding: 8px 0;">No retention policies configured</div>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>WAL Configuration</h3>
      </div>
      <div class="flex gap-4 items-center">
        <div class="form-group" style="width: 200px;">
          <label>Sync Mode</label>
          <select v-model="walConfig.sync_mode" @change="updateWalConfig">
            <option value="every_write">Every Write</option>
            <option value="every_second">Every Second</option>
            <option value="none">None</option>
          </select>
        </div>
        <div class="stat-card" style="padding: 12px 16px;">
          <div class="stat-label">WAL Size</div>
          <div style="font-size: 16px; font-weight: 600; color: var(--text-h);">{{ formatBytes(walConfig.current_size_bytes || 0) }}</div>
        </div>
        <div class="stat-card" style="padding: 12px 16px;">
          <div class="stat-label">Max WAL Size</div>
          <div style="font-size: 16px; font-weight: 600; color: var(--text-h);">{{ formatBytes(walConfig.max_file_size_bytes || 0) }}</div>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Block Management</h3>
        <div class="flex gap-2">
          <button class="btn btn-sm btn-secondary" @click="triggerFlush">Trigger Flush</button>
          <button class="btn btn-sm btn-secondary" @click="triggerCompaction">Trigger Compaction</button>
          <button class="btn btn-sm btn-secondary" @click="loadBlocks">Refresh</button>
        </div>
      </div>
      <div style="margin-bottom: 12px; padding: 12px; background: var(--bg); border-radius: 8px;">
        <div class="flex gap-2 items-center flex-wrap">
          <span style="font-size: 13px; color: var(--text-muted);">Time Range Filter:</span>
          <div class="form-group" style="margin: 0; min-width: 180px;">
            <input
              type="datetime-local"
              v-model="blockFilterStart"
              style="font-size: 12px;"
            />
          </div>
          <span style="color: var(--text-muted);">-</span>
          <div class="form-group" style="margin: 0; min-width: 180px;">
            <input
              type="datetime-local"
              v-model="blockFilterEnd"
              style="font-size: 12px;"
            />
          </div>
          <button class="btn btn-sm btn-primary" @click="loadBlocks">Apply</button>
          <button class="btn btn-sm btn-secondary" @click="clearBlockFilter">Clear</button>
        </div>
      </div>
      <div v-if="blocks.length === 0" style="color: var(--text-muted); font-size: 13px; padding: 8px 0;">No blocks on disk</div>
      <div v-else style="overflow-x: auto;">
        <table>
          <thead>
            <tr>
              <th>Block ID</th>
              <th>Metric</th>
              <th>Time Range</th>
              <th>Series Count</th>
              <th>Compressed Size</th>
              <th>Original Size</th>
              <th>Ratio</th>
              <th>CRC32</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="b in blocks" :key="b.block_id">
              <td style="font-family: var(--mono); font-size: 11px;">{{ b.block_id.substring(0, 8) }}...</td>
              <td>{{ b.metric }}</td>
              <td style="font-family: var(--mono); font-size: 11px;">
                {{ formatTimestamp(b.min_timestamp) }} - {{ formatTimestamp(b.max_timestamp) }}
              </td>
              <td>{{ b.series_count }}</td>
              <td>{{ formatBytes(b.compressed_size) }}</td>
              <td>{{ formatBytes(b.original_size) }}</td>
              <td>{{ b.original_size > 0 ? (b.compressed_size / b.original_size * 100).toFixed(1) + '%' : '-' }}</td>
              <td style="font-family: var(--mono); font-size: 11px;">{{ b.crc32 }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getBlocks, triggerFlush as apiFlush, triggerCompaction as apiCompaction, getWalConfig, updateWalConfig as apiUpdateWal, getRetentionPolicies, createRetentionPolicy } from '../api'

const showRetentionForm = ref(false)
const retentionPolicies = ref([])
const newRetention = ref({ metric: '', ttl_days: 30, downsample_7d_interval_secs: null, downsample_30d_interval_secs: null })
const walConfig = ref({ sync_mode: 'every_second', max_file_size_bytes: 67108864, current_size_bytes: 0 })
const blocks = ref([])
const blockFilterStart = ref('')
const blockFilterEnd = ref('')

function formatBytes(bytes) {
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + ' GB'
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + ' MB'
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return bytes + ' B'
}

function formatInterval(secs) {
  if (secs >= 86400) return (secs / 86400) + 'd'
  if (secs >= 3600) return (secs / 3600) + 'h'
  if (secs >= 60) return (secs / 60) + 'm'
  return secs + 's'
}

function formatTimestamp(ns) {
  if (!ns) return '-'
  return new Date(ns / 1_000_000).toLocaleString()
}

function datetimeToNanos(dtStr) {
  if (!dtStr) return null
  const ms = new Date(dtStr).getTime()
  if (isNaN(ms)) return null
  return ms * 1_000_000
}

async function addRetention() {
  try {
    await createRetentionPolicy(newRetention.value)
    retentionPolicies.value.push({ ...newRetention.value })
    newRetention.value = { metric: '', ttl_days: 30, downsample_7d_interval_secs: null, downsample_30d_interval_secs: null }
    showRetentionForm.value = false
  } catch (e) {
    console.error('Failed to add retention policy:', e)
  }
}

async function updateWalConfig() {
  try {
    await apiUpdateWal({ sync_mode: walConfig.value.sync_mode })
  } catch (e) {
    console.error('Failed to update WAL config:', e)
  }
}

function clearBlockFilter() {
  blockFilterStart.value = ''
  blockFilterEnd.value = ''
  loadBlocks()
}

async function loadBlocks() {
  try {
    const params = {}
    const startNs = datetimeToNanos(blockFilterStart.value)
    const endNs = datetimeToNanos(blockFilterEnd.value)
    if (startNs !== null) params.start_time = startNs
    if (endNs !== null) params.end_time = endNs
    const resp = await getBlocks(params)
    blocks.value = resp.blocks || []
  } catch (e) {
    console.error('Failed to load blocks:', e)
  }
}

async function triggerFlush() {
  try {
    await apiFlush()
    await loadBlocks()
  } catch (e) {
    console.error('Flush failed:', e)
  }
}

async function triggerCompaction() {
  try {
    await apiCompaction()
    await loadBlocks()
  } catch (e) {
    console.error('Compaction failed:', e)
  }
}

onMounted(async () => {
  try {
    const [wc, rp] = await Promise.all([getWalConfig(), getRetentionPolicies()])
    walConfig.value = { ...walConfig.value, ...wc }
    retentionPolicies.value = Array.isArray(rp) ? rp : []
  } catch (e) {
    console.error('Failed to load config:', e)
  }
  loadBlocks()
})
</script>
