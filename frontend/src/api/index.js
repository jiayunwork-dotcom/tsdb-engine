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

export async function getAlertRules() {
  const resp = await api.get('/alerts/rules')
  return resp.data
}

export async function createAlertRule(rule) {
  const resp = await api.post('/alerts/rules', rule)
  return resp.data
}

export async function updateAlertRule(id, rule) {
  const resp = await api.put(`/alerts/rules/${id}`, rule)
  return resp.data
}

export async function deleteAlertRule(id) {
  const resp = await api.delete(`/alerts/rules/${id}`)
  return resp.data
}

export async function enableAlertRule(id) {
  const resp = await api.post(`/alerts/rules/${id}/enable`)
  return resp.data
}

export async function disableAlertRule(id) {
  const resp = await api.post(`/alerts/rules/${id}/disable`)
  return resp.data
}

export async function getAlertEvents(params) {
  const resp = await api.get('/alerts/events', { params: params || {} })
  return resp.data
}

export async function getActiveAlerts() {
  const resp = await api.get('/alerts/active')
  return resp.data
}

export async function acknowledgeAlertEvent(id, body) {
  const resp = await api.post(`/alerts/events/${id}/ack`, body)
  return resp.data
}

export async function getAlertTemplates() {
  const resp = await api.get('/alerts/templates')
  return resp.data
}

export function createAlertWs() {
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const url = `${proto}//${window.location.host}/ws/alerts`
  return new WebSocket(url)
}
