<script setup lang="ts">
import ConfigFormPanel from "./components/ConfigFormPanel.vue";
import DataGenPanel from "./components/DataGenPanel.vue";
import StatusLogPanel from "./components/StatusLogPanel.vue";
import SentDataPanel from "./components/SentDataPanel.vue";
</script>

<template>
  <div class="app">
    <header><h1>PmuSim 子站</h1></header>
    <main class="layout">
      <div class="left">
        <ConfigFormPanel />
        <DataGenPanel />
        <StatusLogPanel />
      </div>
      <div class="right">
        <SentDataPanel />
      </div>
    </main>
  </div>
</template>

<style>
body { margin: 0; font-family: system-ui, sans-serif; }
.app { display: flex; flex-direction: column; height: 100vh; }
header { padding: 8px 16px; border-bottom: 1px solid #ddd; }
header h1 { font-size: 16px; margin: 0; }
.layout { display: grid; grid-template-columns: 380px 1fr; flex: 1; overflow: hidden; }
.left { overflow: auto; border-right: 1px solid #eee; }
.right { overflow: auto; }
</style>
