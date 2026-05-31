<script setup lang="ts">
import { masterPeer, streaming, sentCount } from "../composables/useSubEvents";
import { useEventLog } from "../composables/useEventLog";

const { events } = useEventLog();
</script>

<template>
  <section class="panel">
    <h3>状态</h3>
    <p>主站: {{ masterPeer ?? "未连接" }}</p>
    <p>推流: {{ streaming ? "进行中" : "停止" }} · 已发 {{ sentCount }} 帧</p>
    <h4>事件日志</h4>
    <ul class="log">
      <li v-for="(e, i) in events" :key="i" :class="{ error: e.kind === 'error' }">
        {{ e.time }} {{ e.message }}
      </li>
    </ul>
  </section>
</template>

<style scoped>
.panel { padding: 12px; }
.log { max-height: 240px; overflow: auto; font-size: 12px; font-family: monospace; list-style: none; padding-left: 0; margin: 0; }
.error { color: #d33; }
</style>
