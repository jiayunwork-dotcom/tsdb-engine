import axios from 'axios'

const api = axios.create({
  baseURL: '/api',
  timeout: 30000,
})

export async function writeData(text) {
  const resp = await api.post('/write', text, {
    headers: { 'Content-Type': 'text/plain' },
  })
  return resp.data
}

export async function deleteData(body) {
  const resp = await api.post('/delete', body)
  return resp.data
}

export async function queryData(query) {
  const resp = await api.post('/query', query)
  return resp.data
}

export async function getMetrics() {
  const resp = await api.get('/metrics')
  return resp.data
}

export async function getTags(metric) {
  const resp = await api.get('/tags', { params: { metric } })
  return resp.data
}

export async function getSeriesCount() {
  const resp = await api.get('/series_count')
  return resp.data
}

export async function getHealth() {
  const resp = await api.get('/health')
  return resp.data
}

export async function getBlocks(params) {
  const resp = await api.get('/blocks', { params: params || {} })
  return resp.data
}

export async function triggerFlush() {
  const resp = await api.post('/flush')
  return resp.data
}

export async function triggerCompaction() {
  const resp = await api.post('/compaction')
  return resp.data
}

export async function getWalConfig() {
  const resp = await api.get('/config/wal')
  return resp.data
}

export async function updateWalConfig(config) {
  const resp = await api.post('/config/wal', config)
  return resp.data
}

export async function getRetentionPolicies() {
  const resp = await api.get('/config/retention')
  return resp.data
}

export async function createRetentionPolicy(policy) {
  const resp = await api.post('/config/retention', policy)
  return resp.data
}

export async function getAlerts() {
  const resp = await api.get('/alerts')
  return resp.data
}

export async function createAlert(alert) {
  const resp = await api.post('/alerts', alert)
  return resp.data
}

export async function getAlertHistory() {
  const resp = await api.get('/alerts/history')
  return resp.data
}
