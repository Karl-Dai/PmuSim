<script setup lang="ts">
import { masterPeer, streaming, sentCount } from "../composables/useSubEvents";
import { useEventLog } from "../composables/useEventLog";
import { t } from "../i18n";

const { events } = useEventLog();
</script>

<template>
  <section class="panel">
    <h3>{{ t("status.title") }}</h3>
    <p>{{ t("status.master", { peer: masterPeer ?? t("status.notConnected") }) }}</p>
    <p>{{ t("status.streaming", { state: streaming ? t("status.streamingOn") : t("status.streamingOff"), count: sentCount }) }}</p>
    <h4>{{ t("status.eventLog") }}</h4>
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
