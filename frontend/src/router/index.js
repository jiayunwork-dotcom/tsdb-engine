import { createRouter, createWebHistory } from 'vue-router'

const routes = [
  { path: '/', name: 'Dashboard', component: () => import('../views/Dashboard.vue') },
  { path: '/explore', name: 'Explore', component: () => import('../views/Explore.vue') },
  { path: '/query', name: 'Query', component: () => import('../views/QueryWorkbench.vue') },
  { path: '/admin', name: 'Admin', component: () => import('../views/Admin.vue') },
  { path: '/alerts', name: 'Alerts', component: () => import('../views/Alerts.vue') },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

export default router
