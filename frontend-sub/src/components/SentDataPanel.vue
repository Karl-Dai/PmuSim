<script setup lang="ts">
import { lastData } from "../composables/useSubEvents";
import { t } from "../i18n";
</script>

<template>
  <section class="panel">
    <h3>{{ t("data.title") }}</h3>
    <template v-if="lastData">
      <p>SOC={{ lastData.soc }} FRACSEC={{ lastData.fracsec }} STAT=0x{{ lastData.stat.toString(16) }}</p>
      <p>FREQ={{ lastData.freq }} DFREQ={{ lastData.dfreq }}</p>
      <table>
        <thead><tr><th>{{ t("data.phasor") }}</th><th>{{ t("data.real") }}</th><th>{{ t("data.imag") }}</th></tr></thead>
        <tbody>
          <tr v-for="(ph, i) in lastData.phasors" :key="i">
            <td>PH{{ i }}</td><td>{{ ph[0].toFixed(1) }}</td><td>{{ ph[1].toFixed(1) }}</td>
          </tr>
        </tbody>
      </table>
      <p>{{ t("data.analog") }}: {{ lastData.analog.join(", ") }}</p>
      <p>{{ t("data.digital") }}: {{ lastData.digital.map(d => "0x" + d.toString(16)).join(", ") }}</p>
    </template>
    <p v-else>{{ t("data.empty") }}</p>
  </section>
</template>

<style scoped>
.panel { padding: 12px; }
table { width: 100%; font-size: 12px; }
</style>
