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
          <span style="font-size: 11px; color: var(--text-muted);">{{ checkedMetrics.length }}/5 selected</span>
        </div>
        <div v-if="loading" style="color: var(--text-muted); font-size: 13px;">Loading...</div>
        <div v-else-if="metrics.length === 0" style="color: var(--text-muted); font-size: 13px;">No metrics found</div>
        <div v-else>
          <div v-for="(metric, idx) in metrics" :key="metric" class="tree-item">
            <div class="tree-label" style="display: flex; align-items: center; gap: 6px;">
              <input
                type="checkbox"
                :checked="checkedMetrics.includes(metric)"
                :disabled="!checkedMetrics.includes(metric) && checkedMetrics.length >= 5"
                @change="toggleMetricCheck(metric)"
                style="accent-color: #6366f1;"
              />
              <span
                v-if="checkedMetrics.includes(metric)"
                class="color-dot"
                :style="{ backgroundColor: metricColors[checkedMetrics.indexOf(metric)] }"
              ></span>
              <span
                style="cursor: pointer;"
                :class="{ active: expandedMetric === metric }"
                @click="expandMetric(metric)"
              >{{ metric }}</span>
            </div>
            <div v-if="expandedMetric === metric && metricTags[metric]" class="tree-children">
              <div v-for="[tagKey, tagValues] in metricTags[metric]" :key="tagKey" class="tree-item">
                <div class="tree-label" style="font-size: 12px; color: var(--text-muted);">
                  {{ tagKey }}
                </div>
                <div style="padding: 4px 0 4px 8px;">
                  <span
                    v-for="val in tagValues.slice(0, 20)"
                    :key="val"
                    class="tag-chip"
                    :class="{ selected: metricFilterTags[metric] && metricFilterTags[metric][tagKey] === val }"
                    @click="toggleTag(metric, tagKey, val)"
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
                  @click="selectedRange = range.value; fetchAllData()"
                >{{ range.label }}</button>
              </div>
              <select v-model="selectedAgg" @change="fetchAllData" style="width: 100px;">
                <option value="">Raw</option>
                <option value="avg">AVG</option>
                <option value="max">MAX</option>
                <option value="min">MIN</option>
                <option value="sum">SUM</option>
                <option value="count">COUNT</option>
                <option value="p99">P99</option>
                <option value="rate">Rate</option>
              </select>
              <select v-if="selectedAgg" v-model="selectedGroupBy" @change="fetchAllData" style="width: 80px;">
                <option value="10s">10s</option>
                <option value="1m">1m</option>
                <option value="5m">5m</option>
                <option value="1h">1h</option>
                <option value="1d">1d</option>
              </select>
            </div>
          </div>
          <div v-if="checkedMetrics.length === 0" style="color: var(--text-muted); text-align: center; padding: 60px 0;">
            Select up to 5 metrics from the left panel to overlay on chart
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
import { ref, reactive, onMounted } from 'vue'
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
const loading = ref(true)
const checkedMetrics = ref([])
const expandedMetric = ref('')
const metricTags = reactive({})
const metricFilterTags = reactive({})
const metricLatestValues = reactive({})
const selectedRange = ref('1h')
const selectedAgg = ref('')
const selectedGroupBy = ref('1m')
const chartData = ref({ labels: [], datasets: [] })

const metricColors = ['#6366f1', '#10b981', '#f59e0b', '#ef4444', '#3b82f6']

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
  plugins: {
    legend: {
      display: true,
      labels: {
        color: '#6b7280',
        generateLabels(chart) {
          return chart.data.datasets.map((ds, i) => ({
            text: `${ds.metricName}: ${ds.latestValue != null ? ds.latestValue.toFixed(2) : '-'}`,
            fillStyle: ds.borderColor,
            strokeStyle: ds.borderColor,
            lineWidth: 2,
            hidden: !chart.isDatasetVisible(i),
            datasetIndex: i,
          }))
        },
      },
    },
  },
  scales: {
    x: { grid: { color: 'rgba(255,255,255,0.05)' }, ticks: { color: '#6b7280', maxTicksLimit: 10 } },
    y: { grid: { color: 'rgba(255,255,255,0.05)' }, ticks: { color: '#6b7280' }, beginAtZero: false },
  },
}

function getTimeRange() {
  const now = Date.now() * 1_000_000
  const map = { '15m': 900, '1h': 3600, '6h': 21600, '24h': 86400, '7d': 604800 }
  const secs = map[selectedRange.value] || 3600
  return { start_time: now - secs * 1_000_000_000, end_time: now }
}

function toggleMetricCheck(metric) {
  const idx = checkedMetrics.value.indexOf(metric)
  if (idx >= 0) {
    checkedMetrics.value.splice(idx, 1)
    delete metricLatestValues[metric]
    delete metricFilterTags[metric]
    rebuildChart()
  } else {
    if (checkedMetrics.value.length >= 5) return
    checkedMetrics.value.push(metric)
    fetchMetricData(metric)
  }
}

async function expandMetric(metric) {
  if (expandedMetric.value === metric) {
    expandedMetric.value = ''
    return
  }
  expandedMetric.value = metric
  if (!metricTags[metric]) {
    try {
      const resp = await getTags(metric)
      metricTags[metric] = resp.tags || []
    } catch (e) {
      metricTags[metric] = []
    }
  }
}

function toggleTag(metric, key, value) {
  if (!metricFilterTags[metric]) {
    metricFilterTags[metric] = {}
  }
  if (metricFilterTags[metric][key] === value) {
    delete metricFilterTags[metric][key]
  } else {
    metricFilterTags[metric][key] = value
  }
  metricFilterTags[metric] = { ...metricFilterTags[metric] }
  if (checkedMetrics.value.includes(metric)) {
    fetchMetricData(metric)
  }
}

async function fetchMetricData(metric) {
  const { start_time, end_time } = getTimeRange()
  const colorIdx = checkedMetrics.value.indexOf(metric)
  const query = {
    metric,
    tags: metricFilterTags[metric] ? { ...metricFilterTags[metric] } : {},
    start_time,
    end_time,
    field: 'value',
  }
  if (selectedAgg.value) query.aggregation = selectedAgg.value
  if (selectedAgg.value && selectedGroupBy.value) query.group_by = selectedGroupBy.value

  try {
    const result = await queryData(query)
    const allValues = (result.series || []).flatMap(s => s.values)
    const latest = allValues.length > 0 ? allValues[allValues.length - 1][1] : null
    metricLatestValues[metric] = latest

    const datasets = (result.series || []).map((s, si) => {
      const tagLabel = Object.entries(s.tags).map(([k, v]) => `${k}=${v}`).join(', ')
      const label = checkedMetrics.value.length > 1
        ? `${metric}${tagLabel ? ' ' + tagLabel : ''}`
        : (tagLabel || metric)
      return {
        metricName: metric,
        latestValue: latest,
        label,
        data: s.values.map(([ts, v]) => ({ x: new Date(ts / 1_000_000).toLocaleTimeString(), y: v })),
        borderColor: metricColors[colorIdx % metricColors.length],
        backgroundColor: 'transparent',
        tension: 0.3,
        pointRadius: 1,
        borderWidth: 2,
      }
    })

    metricDatasets[metric] = datasets
    rebuildChart()
  } catch (e) {
    console.error('Query error:', e)
    metricDatasets[metric] = []
    rebuildChart()
  }
}

const metricDatasets = reactive({})

function rebuildChart() {
  const ordered = []
  for (const m of checkedMetrics.value) {
    if (metricDatasets[m]) {
      ordered.push(...metricDatasets[m])
    }
  }

  const allLabels = [...new Set(ordered.flatMap(ds => ds.data.map(d => d.x)))].sort()

  chartData.value = {
    labels: allLabels,
    datasets: ordered.map(ds => ({
      ...ds,
      data: allLabels.map(label => {
        const point = ds.data.find(d => d.x === label)
        return point ? point.y : null
      }),
    })),
  }
}

async function fetchAllData() {
  await Promise.all(checkedMetrics.value.map(m => fetchMetricData(m)))
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

<style scoped>
.color-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.tree-label.active {
  color: var(--text-h);
  font-weight: 600;
}
</style>
