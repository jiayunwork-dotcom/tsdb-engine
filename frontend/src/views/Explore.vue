<template>
  <div>
    <div class="page-header">
      <h1>Data Explorer</h1>
      <p>Browse metrics and visualize time series data</p>
    </div>

    <div class="flex gap-4" style="align-items: flex-start;">
      <div class="card" style="width: 280px; flex-shrink: 0;">
        <div class="card-header">
          <h3>Metrics</h3>
        </div>
        <div v-if="loading" style="color: var(--text-muted); font-size: 13px;">Loading...</div>
        <div v-else-if="metrics.length === 0" style="color: var(--text-muted); font-size: 13px;">No metrics found</div>
        <div v-else>
          <div v-for="metric in metrics" :key="metric" class="tree-item">
            <div class="tree-label" :class="{ active: selectedMetric === metric }" @click="selectMetric(metric)">
              <span style="margin-right: 6px;">&#9656;</span> {{ metric }}
            </div>
            <div v-if="selectedMetric === metric && expandedTags" class="tree-children">
              <div v-for="[tagKey, tagValues] in tags" :key="tagKey" class="tree-item">
                <div class="tree-label" style="font-size: 12px; color: var(--text-muted);">
                  {{ tagKey }}
                </div>
                <div style="padding: 4px 0 4px 8px;">
                  <span
                    v-for="val in tagValues.slice(0, 20)"
                    :key="val"
                    class="tag-chip"
                    :class="{ selected: selectedTags[tagKey] === val }"
                    @click="toggleTag(tagKey, val)"
                  >{{ val }}</span>
                  <span v-if="tagValues.length > 20" style="font-size: 11px; color: var(--text-muted);">
                    +{{ tagValues.length - 20 }} more
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div style="flex: 1;">
        <div class="card">
          <div class="card-header">
            <h3>Time Series</h3>
            <div class="flex gap-2 items-center">
              <div class="time-range-selector">
                <button
                  v-for="range in timeRanges"
                  :key="range.value"
                  :class="{ active: selectedRange === range.value }"
                  @click="selectedRange = range.value; fetchData()"
                >{{ range.label }}</button>
              </div>
              <select v-model="selectedAgg" @change="fetchData" style="width: 100px;">
                <option value="">Raw</option>
                <option value="avg">AVG</option>
                <option value="max">MAX</option>
                <option value="min">MIN</option>
                <option value="sum">SUM</option>
                <option value="count">COUNT</option>
                <option value="p99">P99</option>
                <option value="rate">Rate</option>
              </select>
              <select v-if="selectedAgg" v-model="selectedGroupBy" @change="fetchData" style="width: 80px;">
                <option value="10s">10s</option>
                <option value="1m">1m</option>
                <option value="5m">5m</option>
                <option value="1h">1h</option>
                <option value="1d">1d</option>
              </select>
            </div>
          </div>
          <div v-if="!selectedMetric" style="color: var(--text-muted); text-align: center; padding: 60px 0;">
            Select a metric from the left panel to view data
          </div>
          <div v-else-if="chartData.labels.length > 0" class="chart-container">
            <Line :data="chartData" :options="chartOptions" />
          </div>
          <div v-else style="color: var(--text-muted); text-align: center; padding: 60px 0;">
            No data available for the selected time range
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
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
import { getMetrics, getTags, queryData } from '../api'

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend)

const metrics = ref([])
const tags = ref([])
const loading = ref(true)
const selectedMetric = ref('')
const expandedTags = ref(false)
const selectedTags = ref({})
const selectedRange = ref('1h')
const selectedAgg = ref('')
const selectedGroupBy = ref('1m')
const chartData = ref({ labels: [], datasets: [] })

const timeRanges = [
  { label: '15m', value: '15m' },
  { label: '1h', value: '1h' },
  { label: '6h', value: '6h' },
  { label: '24h', value: '24h' },
  { label: '7d', value: '7d' },
]

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: { legend: { display: true, labels: { color: '#6b7280' } } },
  scales: {
    x: { grid: { color: 'rgba(255,255,255,0.05)' }, ticks: { color: '#6b7280', maxTicksLimit: 10 } },
    y: { grid: { color: 'rgba(255,255,255,0.05)' }, ticks: { color: '#6b7280' }, beginAtZero: false },
  },
}

const colors = ['#6366f1', '#10b981', '#f59e0b', '#ef4444', '#3b82f6', '#8b5cf6', '#ec4899']

function getTimeRange() {
  const now = Date.now() * 1_000_000
  const map = { '15m': 900, '1h': 3600, '6h': 21600, '24h': 86400, '7d': 604800 }
  const secs = map[selectedRange.value] || 3600
  return { start_time: now - secs * 1_000_000_000, end_time: now }
}

async function selectMetric(metric) {
  selectedMetric.value = metric
  expandedTags.value = true
  selectedTags.value = {}
  try {
    const resp = await getTags(metric)
    tags.value = resp.tags || []
  } catch (e) {
    tags.value = []
  }
  fetchData()
}

function toggleTag(key, value) {
  if (selectedTags.value[key] === value) {
    delete selectedTags.value[key]
  } else {
    selectedTags.value[key] = value
  }
  selectedTags.value = { ...selectedTags.value }
  fetchData()
}

async function fetchData() {
  if (!selectedMetric.value) return

  const { start_time, end_time } = getTimeRange()
  const query = {
    metric: selectedMetric.value,
    tags: { ...selectedTags.value },
    start_time,
    end_time,
    field: 'value',
  }
  if (selectedAgg.value) query.aggregation = selectedAgg.value
  if (selectedAgg.value && selectedGroupBy.value) query.group_by = selectedGroupBy.value

  try {
    const result = await queryData(query)
    const datasets = (result.series || []).map((s, i) => ({
      label: Object.entries(s.tags).map(([k, v]) => `${k}=${v}`).join(', ') || selectedMetric.value,
      data: s.values.map(([ts, v]) => ({ x: new Date(ts / 1_000_000).toLocaleTimeString(), y: v })),
      borderColor: colors[i % colors.length],
      backgroundColor: 'transparent',
      tension: 0.3,
      pointRadius: 1,
      borderWidth: 2,
    }))

    const allLabels = [...new Set(datasets.flatMap(ds => ds.data.map(d => d.x)))].sort()

    chartData.value = {
      labels: allLabels,
      datasets: datasets.map(ds => ({
        ...ds,
        data: allLabels.map(label => {
          const point = ds.data.find(d => d.x === label)
          return point ? point.y : null
        }),
      })),
    }
  } catch (e) {
    console.error('Query error:', e)
    chartData.value = { labels: [], datasets: [] }
  }
}

onMounted(async () => {
  try {
    const resp = await getMetrics()
    metrics.value = resp.metrics || []
  } catch (e) {
    console.error('Failed to load metrics:', e)
  } finally {
    loading.value = false
  }
})
</script>
