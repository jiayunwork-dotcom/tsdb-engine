<template>
  <div>
    <div class="page-header">
      <h1>Query Workbench</h1>
      <p>Execute queries using simplified InfluxQL syntax</p>
    </div>

    <div class="card">
      <textarea
        class="query-editor"
        v-model="queryText"
        placeholder="SELECT avg(value) FROM cpu WHERE host='server01' GROUP BY time(1m)"
        @keydown.ctrl.enter="executeQuery"
        @keydown.meta.enter="executeQuery"
        rows="4"
      ></textarea>
      <div class="flex gap-2 items-center" style="margin-top: 12px;">
        <button class="btn btn-primary" @click="executeQuery">Execute (Ctrl+Enter)</button>
        <button class="btn btn-secondary" @click="clearQuery">Clear</button>
        <span v-if="queryTime" style="font-size: 12px; color: var(--text-muted);">
          Query took {{ queryTime }}ms
        </span>
      </div>
    </div>

    <div class="flex gap-4" style="align-items: flex-start;">
      <div style="flex: 1;">
        <div class="card" v-if="queryResult">
          <div class="card-header">
            <h3>Chart View</h3>
            <span v-if="queryResult.truncated" class="badge badge-warning">Truncated</span>
          </div>
          <div class="chart-container">
            <Line :data="resultChartData" :options="chartOptions" />
          </div>
        </div>
      </div>
    </div>

    <div class="card" v-if="queryResult && tableData.length > 0">
      <div class="card-header">
        <h3>Table View</h3>
      </div>
      <div style="overflow-x: auto;">
        <table>
          <thead>
            <tr>
              <th>Time</th>
              <th v-for="col in tableColumns" :key="col">{{ col }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, i) in tableData.slice(0, 500)" :key="i">
              <td style="font-family: var(--mono); font-size: 12px;">{{ row.time }}</td>
              <td v-for="col in tableColumns" :key="col" style="font-family: var(--mono); font-size: 12px;">
                {{ row[col] }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Query History</h3>
        <button class="btn btn-sm btn-secondary" @click="queryHistory = []">Clear</button>
      </div>
      <div v-if="queryHistory.length === 0" style="color: var(--text-muted); font-size: 13px;">No queries yet</div>
      <div v-for="(q, i) in queryHistory" :key="i" style="padding: 8px 0; border-bottom: 1px solid var(--border);">
        <div class="flex justify-between items-center">
          <code style="cursor: pointer; font-size: 12px;" @click="queryText = q.query">{{ q.query }}</code>
          <span style="font-size: 11px; color: var(--text-muted);">{{ q.time }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import { Line } from 'vue-chartjs'
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
} from 'chart.js'
import { queryData } from '../api'

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend)

const queryText = ref('')
const queryResult = ref(null)
const queryTime = ref(null)
const queryHistory = ref([])

const colors = ['#6366f1', '#10b981', '#f59e0b', '#ef4444', '#3b82f6', '#8b5cf6', '#ec4899']

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: { legend: { display: true, labels: { color: '#6b7280' } } },
  scales: {
    x: { grid: { color: 'rgba(255,255,255,0.05)' }, ticks: { color: '#6b7280', maxTicksLimit: 10 } },
    y: { grid: { color: 'rgba(255,255,255,0.05)' }, ticks: { color: '#6b7280' } },
  },
}

const resultChartData = computed(() => {
  if (!queryResult.value) return { labels: [], datasets: [] }
  const series = queryResult.value.series || []
  const datasets = series.map((s, i) => ({
    label: Object.entries(s.tags).map(([k, v]) => `${k}=${v}`).join(', ') || 'value',
    data: s.values.map(([ts, v]) => v),
    borderColor: colors[i % colors.length],
    backgroundColor: 'transparent',
    tension: 0.3,
    pointRadius: 1,
    borderWidth: 2,
  }))
  const labels = (series[0]?.values || []).map(([ts]) => new Date(ts / 1_000_000).toLocaleTimeString())
  return { labels, datasets }
})

const tableColumns = computed(() => {
  if (!queryResult.value) return []
  const series = queryResult.value.series || []
  const cols = new Set()
  series.forEach(s => {
    Object.keys(s.tags).forEach(k => cols.add(k))
  })
  return [...cols, 'value']
})

const tableData = computed(() => {
  if (!queryResult.value) return []
  const series = queryResult.value.series || []
  const rows = []
  series.forEach(s => {
    s.values.forEach(([ts, v]) => {
      rows.push({
        time: new Date(ts / 1_000_000).toLocaleString(),
        ...s.tags,
        value: typeof v === 'number' ? v.toFixed(4) : v,
      })
    })
  })
  return rows
})

function parseSimpleQuery(text) {
  const now = Date.now() * 1_000_000
  const defaultRange = { start_time: now - 3600 * 1_000_000_000, end_time: now }

  let metric = ''
  let tags = {}
  let aggregation = null
  let groupBy = null
  let field = 'value'

  const selectMatch = text.match(/SELECT\s+(\w+)\((\w+)\)\s+FROM\s+(\w+)/i)
  if (selectMatch) {
    aggregation = selectMatch[1].toLowerCase()
    field = selectMatch[2]
    metric = selectMatch[3]
  } else {
    const fromMatch = text.match(/FROM\s+(\w+)/i)
    if (fromMatch) metric = fromMatch[1]
  }

  if (!metric) {
    const parts = text.trim().split(/\s+/)
    if (parts.length > 0) metric = parts[0]
  }

  const whereMatch = text.match(/WHERE\s+(.+?)(?:\s+GROUP|\s+ORDER|\s+LIMIT|$)/i)
  if (whereMatch) {
    const conditions = whereMatch[1].split(/\s+AND\s+/i)
    conditions.forEach(cond => {
      const kv = cond.match(/(\w+)\s*=\s*'?([^']*)'?/)
      if (kv) tags[kv[1]] = kv[2]
    })
  }

  const groupMatch = text.match(/GROUP\s+BY\s+time\((\w+)\)/i)
  if (groupMatch) groupBy = groupMatch[1]

  const timeMatch = text.match(/time\s*>\s*now\(\)\s*-\s*(\d+)([smhd])/i)
  if (timeMatch) {
    const num = parseInt(timeMatch[1])
    const unit = timeMatch[2]
    const map = { s: 1, m: 60, h: 3600, d: 86400 }
    const secs = num * (map[unit] || 60)
    defaultRange.start_time = now - secs * 1_000_000_000
  }

  return {
    metric,
    tags,
    start_time: defaultRange.start_time,
    end_time: defaultRange.end_time,
    field,
    aggregation,
    group_by: groupBy,
  }
}

async function executeQuery() {
  const q = queryText.value.trim()
  if (!q) return

  const startMs = performance.now()
  try {
    const queryObj = parseSimpleQuery(q)
    const result = await queryData(queryObj)
    queryResult.value = result
    queryTime.value = Math.round(performance.now() - startMs)

    queryHistory.value.unshift({
      query: q,
      time: new Date().toLocaleTimeString(),
    })
    if (queryHistory.value.length > 20) {
      queryHistory.value = queryHistory.value.slice(0, 20)
    }
  } catch (e) {
    console.error('Query error:', e)
    queryResult.value = null
  }
}

function clearQuery() {
  queryText.value = ''
  queryResult.value = null
  queryTime.value = null
}
</script>
