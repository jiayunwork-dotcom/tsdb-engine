<template>
  <div>
    <div class="page-header">
      <h1>Dashboard</h1>
      <p>Real-time overview of your TSDB engine</p>
    </div>

    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-label">Write QPS</div>
        <div class="stat-value accent">{{ formatNumber(health.write_qps || 0) }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Active Series</div>
        <div class="stat-value success">{{ formatNumber(seriesCount) }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Disk Usage</div>
        <div class="stat-value warning">{{ formatBytes(health.wal_size_bytes + (health.block_count || 0) * 1024 * 1024) }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Memory Usage</div>
        <div class="stat-value">{{ formatBytes(health.memory_usage_bytes || 0) }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Block Count</div>
        <div class="stat-value">{{ health.block_count || 0 }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">Query P99 Latency</div>
        <div class="stat-value">{{ formatDuration(health.query_latency_p99_us || 0) }}</div>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <h3>Write QPS (Last Hour)</h3>
        <span class="badge badge-success">Live</span>
      </div>
      <div class="chart-container">
        <Line :data="qpsChartData" :options="chartOptions" />
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, computed } from 'vue'
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
  Filler,
} from 'chart.js'
import { getHealth, getSeriesCount } from '../api'

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend, Filler)

const health = ref({})
const seriesCount = ref(0)
const qpsHistory = ref([])
const timeLabels = ref([])
let intervalId = null

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: { display: false },
  },
  scales: {
    x: {
      grid: { color: 'rgba(255,255,255,0.05)' },
      ticks: { color: '#6b7280', maxTicksLimit: 12 },
    },
    y: {
      grid: { color: 'rgba(255,255,255,0.05)' },
      ticks: { color: '#6b7280' },
      beginAtZero: true,
    },
  },
}

const qpsChartData = computed(() => ({
  labels: timeLabels.value,
  datasets: [
    {
      label: 'Write QPS',
      data: qpsHistory.value,
      borderColor: '#6366f1',
      backgroundColor: 'rgba(99, 102, 241, 0.1)',
      fill: true,
      tension: 0.4,
      pointRadius: 2,
      borderWidth: 2,
    },
  ],
}))

function formatNumber(n) {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K'
  return Math.round(n).toString()
}

function formatBytes(bytes) {
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + ' GB'
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + ' MB'
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return bytes + ' B'
}

function formatDuration(us) {
  if (us >= 1000000) return (us / 1000000).toFixed(1) + ' s'
  if (us >= 1000) return (us / 1000).toFixed(1) + ' ms'
  return us + ' μs'
}

async function refresh() {
  try {
    const [h, sc] = await Promise.all([getHealth(), getSeriesCount()])
    health.value = h
    seriesCount.value = sc.count

    const now = new Date()
    const label = now.toLocaleTimeString()
    qpsHistory.value.push(h.write_qps || 0)
    timeLabels.value.push(label)

    if (qpsHistory.value.length > 360) {
      qpsHistory.value.shift()
      timeLabels.value.shift()
    }
  } catch (e) {
    console.error('Dashboard refresh error:', e)
  }
}

onMounted(() => {
  refresh()
  intervalId = setInterval(refresh, 10000)
})

onUnmounted(() => {
  if (intervalId) clearInterval(intervalId)
})
</script>
