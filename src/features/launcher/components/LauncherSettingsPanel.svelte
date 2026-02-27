<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { LauncherSettings } from "../models/settings";
  import {
    getTotalRamMb,
    loadLauncherSettings,
    saveLauncherSettings,
  } from "../services/launcher-settings-service";

  export let theme: "dark" | "light" = "dark";
  export let onBack: () => void;
  export let onThemeChange: (theme: "dark" | "light") => void;

  let totalRamMb = 8192;
  let ramMb = 2048;
  let javaArgs = "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions";
  let closeOnLaunch = false;
  let showLogs = true;
  let infoMessage = "";
  let closeTimerId: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    try {
      const realTotalRamMb = await getTotalRamMb();
      if (Number.isFinite(realTotalRamMb) && realTotalRamMb > 0) {
        totalRamMb = Math.round(realTotalRamMb);
      }
    } catch {
      const reportedMemoryGb = (
        navigator as Navigator & { deviceMemory?: number }
      ).deviceMemory;
      if (typeof reportedMemoryGb === "number" && reportedMemoryGb > 0) {
        totalRamMb = Math.round(reportedMemoryGb * 1024);
      }
    }

    ramMb = clampRamToStep(Math.max(1024, Math.round(totalRamMb / 3)));

    try {
      const stored = await loadLauncherSettings();
      ramMb = clampRamToStep(stored.ramMb);
      theme = stored.theme;
      javaArgs = stored.javaArgs;
      closeOnLaunch = stored.closeOnLaunch;
      showLogs = stored.showLogs;
      onThemeChange(theme);
    } catch {
      // No settings file yet or failed to read; keep defaults.
    }
  });

  $: ramLabel = `${ramMb} MB (${(ramMb / 1024).toFixed(1)} GB)`;
  $: totalRamLabel = `${totalRamMb} MB`;

  async function saveSettings(): Promise<void> {
    const payload: LauncherSettings = {
      ramMb: clampRamToStep(ramMb),
      theme,
      javaArgs,
      closeOnLaunch,
      showLogs,
    };

    try {
      await saveLauncherSettings(payload);
      infoMessage = "Настройки сохранены.";
      onThemeChange(theme);
      if (closeTimerId) {
        clearTimeout(closeTimerId);
      }
      closeTimerId = setTimeout(() => {
        onBack();
      }, 700);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Ошибка сохранения.";
      infoMessage = message;
    }
  }

  onDestroy(() => {
    if (closeTimerId) {
      clearTimeout(closeTimerId);
    }
  });

  function clampRamToStep(value: number): number {
    const min = 1024;
    const max = Math.max(min, totalRamMb);
    const step = 256;

    const clamped = Math.min(max, Math.max(min, value));
    return Math.round(clamped / step) * step;
  }
</script>

<section class="panel" aria-label="Настройки лаунчера">
  <header class="panel-head">
    <h2>Настройки лаунчера</h2>
    <button
      type="button"
      class="back"
      on:click={onBack}
      aria-label="Вернуться в главное меню">↩</button
    >
  </header>

  <div class="grid">
    <label for="ram-slider">Выделение ОЗУ</label>
    <input
      id="ram-slider"
      type="range"
      min="1024"
      max={Math.max(1024, totalRamMb)}
      step="256"
      bind:value={ramMb}
    />
    <p class="ram-meta">
      Текущее значение: <strong>{ramLabel}</strong> | Доступно: {totalRamLabel}
    </p>

    <label for="theme">Тема</label>
    <select
      id="theme"
      bind:value={theme}
      on:change={() => onThemeChange(theme)}
    >
      <option value="dark">Темная</option>
      <option value="light">Светлая</option>
    </select>

    <label for="args">Параметры запуска Java</label>
    <textarea id="args" rows="3" bind:value={javaArgs}></textarea>
  </div>

  <div class="checks">
    <label
      ><input type="checkbox" bind:checked={closeOnLaunch} /> Закрывать лаунчер после
      запуска игры</label
    >
    <label
      ><input type="checkbox" bind:checked={showLogs} /> Показывать консоль логов
      при запуске</label
    >
  </div>

  {#if infoMessage}
    <p class="success">{infoMessage}</p>
  {/if}

  <button type="button" class="save" on:click={saveSettings}
    >Сохранить параметры</button
  >
</section>

<style>
  .panel {
    display: grid;
    gap: 14px;
  }

  .panel-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }

  .panel-head h2 {
    margin: 0;
  }

  .back {
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    width: 38px;
    height: 38px;
    border-radius: 10px;
    font-size: 1.1rem;
    cursor: pointer;
    transition:
      transform 0.14s ease,
      border-color 0.14s ease,
      box-shadow 0.14s ease;
  }

  .back:hover {
    transform: scale(1.06);
    border-color: var(--accent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 36%, transparent);
  }

  .grid {
    display: grid;
    gap: 10px;
  }

  label {
    color: var(--text-soft);
    font-size: 0.9rem;
  }

  input,
  select,
  textarea {
    border-radius: 12px;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    font: inherit;
    padding: 11px 12px;
  }

  input[type="range"] {
    padding: 0 10px;
    height: 30px;
    appearance: none;
    -webkit-appearance: none;
    background: var(--surface-main);
    border-radius: 999px;
    cursor: pointer;
  }

  input[type="range"]::-webkit-slider-runnable-track {
    height: 8px;
    border-radius: 999px;
    background: var(--line);
  }

  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    margin-top: -5px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1px solid color-mix(in srgb, var(--accent) 60%, #000 40%);
    background: var(--accent);
    box-shadow: 0 0 0 2px
      color-mix(in srgb, var(--surface-main) 80%, transparent);
  }

  input[type="range"]::-moz-range-track {
    height: 8px;
    border-radius: 999px;
    background: var(--line);
  }

  input[type="range"]::-moz-range-thumb {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1px solid color-mix(in srgb, var(--accent) 60%, #000 40%);
    background: var(--accent);
    box-shadow: 0 0 0 2px
      color-mix(in srgb, var(--surface-main) 80%, transparent);
  }

  input[type="range"]:focus-visible::-webkit-slider-thumb {
    box-shadow:
      0 0 0 2px color-mix(in srgb, var(--surface-main) 80%, transparent),
      0 0 0 5px color-mix(in srgb, var(--accent) 26%, transparent);
  }

  input[type="range"]:focus-visible::-moz-range-thumb {
    box-shadow:
      0 0 0 2px color-mix(in srgb, var(--surface-main) 80%, transparent),
      0 0 0 5px color-mix(in srgb, var(--accent) 26%, transparent);
  }

  .ram-meta {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.9rem;
  }

  textarea {
    resize: vertical;
  }

  .checks {
    display: grid;
    gap: 8px;
    color: var(--text-muted);
  }

  .checks label {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
  }

  .success {
    margin: 0;
    color: #95e3af;
    font-size: 0.9rem;
  }

  .save {
    justify-self: start;
    border-radius: 12px;
    border: none;
    padding: 11px 16px;
    font: inherit;
    font-weight: 700;
    color: #0b1422;
    background: linear-gradient(135deg, #9ef7ce, #84bffb);
    cursor: pointer;
    transition:
      transform 0.14s ease,
      box-shadow 0.14s ease,
      filter 0.14s ease;
  }

  .save:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(70, 182, 171, 0.3);
    filter: brightness(1.05);
  }
</style>
