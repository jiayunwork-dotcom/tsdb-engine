<template>
  <div class="app-layout">
    <nav class="sidebar">
      <div class="sidebar-brand">
        <h2>TSDB Engine</h2>
      </div>
      <ul class="nav-links">
        <li><router-link to="/" exact-active-class="active">Dashboard</router-link></li>
        <li><router-link to="/explore" active-class="active">Data Explore</router-link></li>
        <li><router-link to="/query" active-class="active">Query Workbench</router-link></li>
        <li><router-link to="/admin" active-class="active">System Admin</router-link></li>
        <li><router-link to="/alerts" active-class="active">Alert Rules</router-link></li>
        <li><router-link to="/alerts/events" active-class="active">Alert Events</router-link></li>
      </ul>
    </nav>
    <main class="main-content">
      <router-view />
    </main>
  </div>
</template>

<script setup>
</script>
