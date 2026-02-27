<script lang="ts">
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import { fade, fly, scale } from "svelte/transition";
  import { createEventDispatcher } from "svelte";

  type BuildItem = {
    id: string;
    name: string;
    description?: string;
    imageFile?: string;
    installed?: boolean;
    filters?: string[];
    loader?: string;
    gameVersion?: string;
  };

  type FilterDefinition = {
    id: string;
    label: string;
  };

  type OpenModeRequest = {
    id: number;
    buildId: string;
  };

  export let builds: BuildItem[] = [];
  export let filterDefinitions: FilterDefinition[] = [];
  export let assetImageNames: string[] = [];
  export let bundledAssetUrlsByName: Record<string, string> = {};
  export let runningModeName: string | null = null;
  export let openModeRequest: OpenModeRequest | null = null;

  const dispatch = createEventDispatcher<{ launch: { modeName: string } }>();

  let modeScope: "all" | "installed" = "all";
  let searchValue = "";
  let activeFilter = "all";
  let selectedBuildId: string | null = null;
  let uiRunningModeName: string | null = null;
  let lastPropRunningModeName: string | null = null;
  let lastOpenModeRequestId = 0;
  let brokenImageByBuildId: Record<string, boolean> = {};

  $: availableFilters = [{ id: "all", label: "Все фильтры" }, ...filterDefinitions];

  $: if (!availableFilters.some((item) => item.id === activeFilter)) {
    activeFilter = "all";
  }

  $: selectedBuild = selectedBuildId
    ? builds.find((build) => build.id === selectedBuildId) ?? null
    : null;

  $: selectedBuildIsRunning = Boolean(
    selectedBuild && uiRunningModeName === selectedBuild.name,
  );

  $: if (runningModeName !== lastPropRunningModeName) {
    uiRunningModeName = runningModeName;
    lastPropRunningModeName = runningModeName;
  }

  $: if (openModeRequest && openModeRequest.id !== lastOpenModeRequestId) {
    selectedBuildId = openModeRequest.buildId;
    lastOpenModeRequestId = openModeRequest.id;
  }

  $: filterLabelById = Object.fromEntries(
    filterDefinitions.map((definition) => [definition.id, definition.label]),
  ) as Record<string, string>;

  $: similarBuilds = selectedBuild
    ? builds.filter((build) => {
        if (build.id === selectedBuild.id) {
          return false;
        }

        const currentFilters = selectedBuild.filters ?? [];
        const candidateFilters = build.filters ?? [];

        return candidateFilters.some((filter) => currentFilters.includes(filter));
      })
    : [];

  $: filteredBuilds = builds.filter((build) => {
    if (modeScope === "installed" && !build.installed) {
      return false;
    }

    if (activeFilter !== "all") {
      const modeFilters = build.filters ?? [];
      if (!modeFilters.includes(activeFilter)) {
        return false;
      }
    }

    const query = searchValue.trim().toLowerCase();
    if (!query) {
      return true;
    }

    return build.name.toLowerCase().includes(query);
  });

  function openModeDetails(buildId: string): void {
    selectedBuildId = buildId;
  }

  function closeModeDetails(): void {
    selectedBuildId = null;
  }

  function sharedGenreLabel(build: BuildItem): string {
    if (!selectedBuild) {
      return "Похожий жанр";
    }

    const baseFilters = new Set(selectedBuild.filters ?? []);
    const sharedFilters = (build.filters ?? []).filter((filter) =>
      baseFilters.has(filter),
    );

    if (sharedFilters.length === 0) {
      return "Похожий жанр";
    }

    return sharedFilters
      .map((filter) => filterLabelById[filter] ?? filter)
      .join(", ");
  }

  function launchMode(modeName: string): void {
    dispatch("launch", { modeName });
  }

  function handleLaunchInteraction(modeName: string): void {
    const nextRunningMode = uiRunningModeName === modeName ? null : modeName;
    uiRunningModeName = nextRunningMode;
    launchMode(modeName);
  }

  function imageForBuild(build: BuildItem): string {
    if (brokenImageByBuildId[build.id]) {
      return "";
    }

    if (build.imageFile) {
      return resolveAssetUrl(build.imageFile);
    }

    const matchedByName = findBestFuzzyImage(build.name, assetImageNames);
    if (!matchedByName) {
      return "";
    }

    return resolveAssetUrl(matchedByName);
  }

  function onImageError(buildId: string): void {
    brokenImageByBuildId = {
      ...brokenImageByBuildId,
      [buildId]: true,
    };
  }

  function resolveAssetUrl(fileName: string): string {
    const bundledUrl = bundledAssetUrlsByName[fileName];
    if (bundledUrl) {
      return bundledUrl;
    }

    return `/assets/${fileName}`;
  }

  function findBestFuzzyImage(buildName: string, files: string[]): string | null {
    if (files.length === 0) {
      return null;
    }

    const source = normalizeName(buildName);
    let best: { file: string; score: number } | null = null;

    for (const file of files) {
      const withoutExt = file.replace(/\.[^.]+$/, "");
      const score = sequenceSimilarity(source, normalizeName(withoutExt));
      if (!best || score > best.score) {
        best = { file, score };
      }
    }

    return best && best.score >= 0.55 ? best.file : null;
  }

  function normalizeName(value: string): string {
    return value.toLowerCase().replace(/[^a-zа-яё0-9]/gi, "");
  }

  // Rough SequenceMatcher-like ratio based on LCS length.
  function sequenceSimilarity(a: string, b: string): number {
    if (!a.length && !b.length) {
      return 1;
    }

    const lcs = lcsLength(a, b);
    return (2 * lcs) / (a.length + b.length);
  }

  function lcsLength(a: string, b: string): number {
    const dp = Array.from({ length: a.length + 1 }, () =>
      Array<number>(b.length + 1).fill(0),
    );

    for (let i = 1; i <= a.length; i++) {
      for (let j = 1; j <= b.length; j++) {
        if (a[i - 1] === b[j - 1]) {
          dp[i][j] = dp[i - 1][j - 1] + 1;
        } else {
          dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
        }
      }
    }

    return dp[a.length][b.length];
  }
</script>

<section class="modes-page" aria-label="Список режимов">
  {#key selectedBuildId ?? "modes-list"}
    {#if selectedBuild}
      <section
        class="mode-details-view"
        aria-label={`Режим ${selectedBuild.name}`}
        in:fly={{ x: 24, duration: 240, easing: cubicOut }}
        out:fade={{ duration: 130 }}
      >
        <button type="button" class="back-btn" on:click={closeModeDetails}>Назад</button>

        <article class="mode-details-card">
          <div class="mode-media-column">
            <div class="mode-details-thumb">
              {#if imageForBuild(selectedBuild)}
                <img
                  src={imageForBuild(selectedBuild)}
                  alt={`Изображение режима ${selectedBuild.name}`}
                  on:error={() => onImageError(selectedBuild.id)}
                />
              {:else}
                <div class="thumb-placeholder" aria-hidden="true">
                  <svg viewBox="0 0 24 24">
                    <path d="M3 6h18v12H3z" />
                    <path d="M3 16l5-5 4 4 3-3 6 6" />
                  </svg>
                </div>
              {/if}
            </div>

            <button
              type="button"
              class="launch-detail-btn"
              class:is-running={selectedBuildIsRunning}
              on:click={() => handleLaunchInteraction(selectedBuild.name)}
            >
              {#if selectedBuildIsRunning}
                Закрыть игру
              {:else}
                Запустить режим
              {/if}
            </button>
          </div>

          <div class="mode-details-content">
            <h2>{selectedBuild.name}</h2>
            <p class="mode-details-description">
              {selectedBuild.description ?? "Описание режима пока не добавлено."}
            </p>
          </div>

          <div class="mode-details-meta">
            Загрузчик, версия: {selectedBuild.loader ?? "Forge"}, {selectedBuild.gameVersion ?? "1.20.1"}
          </div>
        </article>

        <section class="similar-section" aria-label="Похожие жанры">
          <h3>Похожие жанры</h3>
          {#if similarBuilds.length === 0}
            <p class="similar-empty">Похожие режимы по жанру пока не найдены.</p>
          {:else}
            <div class="similar-grid">
              {#each similarBuilds as build (build.id)}
                <button
                  type="button"
                  class="similar-card"
                  aria-label={`Открыть похожий режим ${build.name}`}
                  on:click={() => openModeDetails(build.id)}
                  in:scale={{ start: 0.95, duration: 170, easing: cubicOut }}
                  out:fade={{ duration: 120 }}
                >
                  <div class="similar-thumb">
                    {#if imageForBuild(build)}
                      <img
                        src={imageForBuild(build)}
                        alt={`Изображение режима ${build.name}`}
                        on:error={() => onImageError(build.id)}
                      />
                    {:else}
                      <div class="thumb-placeholder" aria-hidden="true">
                        <svg viewBox="0 0 24 24">
                          <path d="M3 6h18v12H3z" />
                          <path d="M3 16l5-5 4 4 3-3 6 6" />
                        </svg>
                      </div>
                    {/if}
                  </div>
                  <div class="similar-name">{build.name}</div>
                  <div class="similar-genre">{sharedGenreLabel(build)}</div>
                </button>
              {/each}
            </div>
          {/if}
        </section>
      </section>
    {:else}
      <section
        class="modes-list-view"
        in:fly={{ x: -20, duration: 220, easing: cubicOut }}
        out:fade={{ duration: 120 }}
      >
        <div class="scope-tabs" role="tablist" aria-label="Область режимов">
          <button
            type="button"
            role="tab"
            class="scope-tab"
            class:active={modeScope === "all"}
            aria-selected={modeScope === "all"}
            on:click={() => (modeScope = "all")}
          >
            Все режимы
          </button>
          <button
            type="button"
            role="tab"
            class="scope-tab"
            class:active={modeScope === "installed"}
            aria-selected={modeScope === "installed"}
            on:click={() => (modeScope = "installed")}
          >
            Установленные
          </button>
        </div>

        <div class="toolbar">
          <label class="search-field" aria-label="Поиск режима">
            <input type="text" bind:value={searchValue} placeholder="Поиск режима по названию" />
          </label>

          <div class="filter-row" aria-label="Фильтры режимов">
            {#each availableFilters as filter (filter.id)}
              <button
                type="button"
                class="filter-chip"
                class:active={activeFilter === filter.id}
                on:click={() => (activeFilter = filter.id)}
              >
                {filter.label}
              </button>
            {/each}
          </div>
        </div>

        {#if filteredBuilds.length === 0}
          <div class="empty-state">По выбранным условиям режимы не найдены.</div>
        {:else}
          <div class="modes-grid">
            {#each filteredBuilds as build (build.id)}
              <button
                type="button"
                class="mode-card"
                aria-label={`Открыть режим ${build.name}`}
                on:click={() => openModeDetails(build.id)}
                in:scale={{ start: 0.95, duration: 180, easing: cubicOut }}
                out:fade={{ duration: 120 }}
                animate:flip={{ duration: 220, easing: cubicOut }}
              >
                <div class="thumb-wrap">
                  {#if imageForBuild(build)}
                    <img
                      src={imageForBuild(build)}
                      alt={`Изображение режима ${build.name}`}
                      on:error={() => onImageError(build.id)}
                    />
                  {:else}
                    <div class="thumb-placeholder" aria-hidden="true">
                      <svg viewBox="0 0 24 24">
                        <path d="M3 6h18v12H3z" />
                        <path d="M3 16l5-5 4 4 3-3 6 6" />
                      </svg>
                    </div>
                  {/if}
                </div>

                <div class="mode-name">{build.name}</div>
                <div class="mode-meta">{build.loader ?? "Forge"} {build.gameVersion ?? "1.20.1"}</div>

                {#if uiRunningModeName === build.name}
                  <span class="installed-badge running">Игра запущена</span>
                {:else if build.installed}
                  <span class="installed-badge">Установлен</span>
                {:else}
                  <span class="installed-badge muted">Не установлен</span>
                {/if}
              </button>
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  {/key}
</section>

<style>
  .modes-page {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
    min-height: 0;
  }

  .modes-list-view {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 2px 2px 0 3px;
    min-height: 0;
    height: 100%;
  }

  .scope-tabs {
    display: inline-flex;
    gap: 8px;
    align-items: center;
  }

  .scope-tab {
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    border-radius: 10px;
    padding: 8px 12px;
    font: inherit;
    cursor: pointer;
    transition:
      transform 0.15s ease,
      border-color 0.15s ease,
      background-color 0.15s ease,
      box-shadow 0.15s ease;
  }

  .scope-tab:hover {
    transform: scale(1.02);
    border-color: var(--accent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 32%, transparent);
  }

  .scope-tab:active {
    transform: scale(0.98);
  }

  .scope-tab.active {
    border-color: var(--accent);
    background: var(--surface-elevated);
  }

  .toolbar {
    display: grid;
    gap: 10px;
    padding: 4px;
  }

  .search-field {
    display: block;
  }

  .search-field input {
    width: 100%;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    border-radius: 10px;
    padding: 10px 12px;
    font: inherit;
    outline: none;
    transition:
      border-color 0.14s ease,
      box-shadow 0.14s ease;
  }

  .search-field input:focus {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .search-field input::placeholder {
    color: var(--text-muted);
  }

  .filter-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .filter-chip {
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    border-radius: 999px;
    padding: 6px 12px;
    font: inherit;
    cursor: pointer;
    transform-origin: center center;
    transition:
      transform 0.14s ease,
      border-color 0.14s ease,
      background-color 0.14s ease,
      box-shadow 0.14s ease;
  }

  .filter-chip:hover {
    transform: scale(1.04);
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .filter-chip:active {
    transform: scale(0.98);
  }

  .filter-chip.active {
    border-color: var(--accent);
    background: var(--surface-elevated);
  }

  .modes-grid {
    position: relative;
    z-index: 0;
    isolation: isolate;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
    align-content: start;
    min-height: 0;
    height: 100%;
    flex: 1 1 auto;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 8px 10px 16px 8px;
    scrollbar-gutter: stable;
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--accent) 45%, var(--line) 55%) var(--surface-main);
  }

  .modes-grid::-webkit-scrollbar {
    width: 10px;
    height: 10px;
  }

  .modes-grid::-webkit-scrollbar-track {
    background: var(--surface-main);
    border-radius: 999px;
  }

  .modes-grid::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--accent) 45%, var(--line) 55%);
    border-radius: 999px;
    border: 2px solid var(--surface-main);
  }

  .modes-grid::-webkit-scrollbar-thumb:hover {
    background: color-mix(in srgb, var(--accent) 65%, var(--line) 35%);
  }

  .mode-card {
    position: relative;
    z-index: 0;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    border-radius: 12px;
    padding: 10px;
    display: grid;
    gap: 8px;
    width: 100%;
    text-align: left;
    font: inherit;
    color: inherit;
    cursor: pointer;
    transform-origin: center center;
    transition:
      transform 0.16s ease,
      border-color 0.16s ease,
      box-shadow 0.16s ease,
      background-color 0.16s ease;
  }

  .mode-card:hover {
    z-index: 3;
    transform: scale(1.03);
    border-color: var(--accent);
    background: var(--surface-elevated);
    box-shadow: 0 12px 22px rgba(9, 16, 26, 0.32);
  }

  .mode-card:focus-visible {
    z-index: 3;
    outline: 2px solid color-mix(in srgb, var(--accent) 75%, transparent);
    outline-offset: 2px;
  }

  .mode-card:active {
    transform: scale(0.99);
  }

  .thumb-wrap,
  .mode-details-thumb {
    width: 100%;
    aspect-ratio: 16 / 9;
    border-radius: 9px;
    overflow: hidden;
    border: 1px solid var(--line);
    background: var(--surface-main);
  }

  .thumb-wrap img,
  .mode-details-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .thumb-placeholder {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }

  .thumb-placeholder svg {
    width: 30px;
    height: 30px;
    stroke: currentColor;
    fill: none;
    stroke-width: 1.8;
  }

  .mode-name {
    display: flex;
    align-items: flex-end;
    font-size: 1rem;
    font-weight: 700;
    line-height: 1.25;
    color: var(--text-main);
    min-height: 2.5em;
  }

  .mode-meta {
    color: var(--text-muted);
    font-size: 0.9rem;
  }

  .installed-badge {
    justify-self: start;
    display: inline-flex;
    border: 1px solid rgba(112, 224, 176, 0.45);
    color: #97e9c5;
    background: rgba(73, 169, 126, 0.15);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 0.74rem;
    line-height: 1.4;
  }

  .installed-badge.running {
    border-color: rgba(255, 115, 115, 0.55);
    color: #ffc2c2;
    background: rgba(215, 66, 66, 0.22);
  }

  .installed-badge.muted {
    border-color: var(--line);
    color: var(--text-muted);
    background: transparent;
  }

  .mode-details-view {
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex: 1 1 auto;
    min-height: 0;
    padding: 4px;
    overflow-x: hidden;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--accent) 52%, var(--line) 48%) var(--surface-main);
  }

  .mode-details-view::-webkit-scrollbar {
    width: 11px;
  }

  .mode-details-view::-webkit-scrollbar-track {
    background: var(--surface-main);
    border-radius: 999px;
  }

  .mode-details-view::-webkit-scrollbar-thumb {
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 70%, var(--surface-elevated) 30%),
      color-mix(in srgb, var(--accent) 44%, var(--line) 56%)
    );
    border-radius: 999px;
    border: 2px solid var(--surface-main);
  }

  .mode-details-view::-webkit-scrollbar-thumb:hover {
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 82%, var(--surface-elevated) 18%),
      color-mix(in srgb, var(--accent) 58%, var(--line) 42%)
    );
  }

  .back-btn {
    justify-self: start;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    border-radius: 10px;
    padding: 8px 12px;
    font: inherit;
    font-weight: 700;
    cursor: pointer;
    transition:
      transform 0.12s ease,
      padding 0.14s ease,
      border-color 0.14s ease,
      box-shadow 0.14s ease;
  }

  .back-btn:hover {
    padding: 9px 13px;
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 35%, transparent);
  }

  .back-btn:active {
    transform: scale(0.98);
  }

  .mode-details-card {
    flex: 0 0 auto;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    border-radius: 12px;
    padding: 10px 12px;
    display: grid;
    grid-template-columns: minmax(0, 1.05fr) minmax(0, 1fr);
    grid-template-rows: auto auto;
    gap: 12px;
    min-height: auto;
    overflow: hidden;
  }

  .mode-media-column {
    display: grid;
    grid-template-rows: auto auto;
    align-content: start;
    gap: 10px;
    min-width: 0;
  }

  .mode-details-thumb {
    height: clamp(180px, 34vh, 360px);
    aspect-ratio: auto;
    border-radius: 11px;
  }

  .mode-details-content {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  .mode-details-content h2 {
    margin: 0;
    font-size: clamp(1.2rem, 2vw, 1.6rem);
    line-height: 1.2;
  }

  .mode-details-description {
    margin: 0;
    white-space: pre-line;
    color: var(--text-soft);
    overflow: hidden auto;
    max-height: 210px;
    padding-right: 4px;
  }

  .launch-detail-btn {
    width: 100%;
    border: none;
    border-radius: 9px;
    padding: 8px 12px;
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

  .launch-detail-btn:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(70, 182, 171, 0.3);
    filter: brightness(1.05);
  }

  .launch-detail-btn:active {
    transform: scale(0.98);
  }

  .launch-detail-btn.is-running {
    color: #ffecec;
    background: linear-gradient(135deg, #ed5f5f, #b32121);
    box-shadow: 0 6px 16px rgba(168, 38, 38, 0.32);
  }

  .launch-detail-btn.is-running:hover {
    box-shadow: 0 8px 18px rgba(168, 38, 38, 0.4);
    filter: brightness(1.08);
  }

  .mode-details-meta {
    grid-column: 1 / -1;
    border-top: 1px solid var(--line);
    padding-top: 12px;
    color: var(--text-muted);
    font-size: 0.9rem;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .similar-section {
    flex: 0 0 auto;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    border-radius: 12px;
    padding: 10px;
    display: grid;
    gap: 10px;
    min-width: 0;
  }

  .similar-section h3 {
    margin: 0;
    font-size: 1rem;
    color: var(--text-main);
  }

  .similar-empty {
    margin: 0;
    color: var(--text-muted);
  }

  .similar-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
    align-content: start;
  }

  .similar-card {
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface-main);
    color: var(--text-main);
    padding: 8px;
    display: grid;
    gap: 6px;
    text-align: left;
    font: inherit;
    cursor: pointer;
    transition:
      transform 0.14s ease,
      border-color 0.14s ease,
      box-shadow 0.14s ease;
  }

  .similar-card:hover {
    transform: scale(1.02);
    border-color: var(--accent);
    box-shadow: 0 8px 16px rgba(9, 16, 26, 0.28);
  }

  .similar-card:active {
    transform: scale(0.98);
  }

  .similar-thumb {
    width: 100%;
    aspect-ratio: 16 / 9;
    border: 1px solid var(--line);
    border-radius: 8px;
    overflow: hidden;
    background: var(--surface-alt);
  }

  .similar-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .similar-name {
    font-size: 0.9rem;
    font-weight: 700;
    line-height: 1.25;
    min-height: 2.2em;
    color: var(--text-main);
  }

  .similar-genre {
    font-size: 0.78rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty-state {
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-muted);
    border-radius: 12px;
    padding: 16px;
  }

  @media (max-width: 1180px) {
    .modes-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 980px) {
    .mode-details-card {
      grid-template-columns: 1fr;
      grid-template-rows: auto 1fr auto;
    }

    .mode-details-description {
      max-height: none;
    }

    .similar-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

  }

  @media (max-width: 780px) {
    .modes-grid {
      grid-template-columns: 1fr;
      max-height: none;
      overflow: visible;
    }

    .similar-grid {
      grid-template-columns: 1fr;
    }

  }
</style>

