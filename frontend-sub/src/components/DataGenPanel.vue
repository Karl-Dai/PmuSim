<script setup lang="ts">
import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { running } from "../composables/useSubEvents";
import { t } from "../i18n";

const gen = reactive({ freq_offset_hz: 0.0, rocof_hz_s: 0.0 });

let timer: number | undefined;
watch(gen, () => {
  clearTimeout(timer);
  timer = window.setTimeout(async () => {
    if (running.value) await invoke("update_gen", { freqOffsetHz: gen.freq_offset_hz, rocofHzS: gen.rocof_hz_s });
  }, 120);
});

async function trigger() { if (running.value) await invoke("fire_trigger"); }
</script>

<template>
  <section class="panel">
    <h3>{{ t("datagen.title") }}</h3>
    <label>{{ t("datagen.freqOffset") }}
      <input type="range" min="-2" max="2" step="0.01" v-model.number="gen.freq_offset_hz" />
      <span>{{ gen.freq_offset_hz.toFixed(2) }}</span>
    </label>
    <label>{{ t("datagen.rocof") }}
      <input type="range" min="-5" max="5" step="0.1" v-model.number="gen.rocof_hz_s" />
      <span>{{ gen.rocof_hz_s.toFixed(1) }}</span>
    </label>
    <button :disabled="!running" @click="trigger">{{ t("datagen.trigger") }}</button>
  </section>
</template>

<style scoped>
.panel { display: flex; flex-direction: column; gap: 8px; padding: 12px; }
label { display: flex; align-items: center; gap: 8px; }
</style>
