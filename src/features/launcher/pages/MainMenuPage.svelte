<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly, scale } from "svelte/transition";
  import BuildShowcase from "../components/BuildShowcase.svelte";
  import AccountSettingsPanel from "../components/AccountSettingsPanel.svelte";
  import LauncherSettingsPanel from "../components/LauncherSettingsPanel.svelte";

  type SessionUser = {
    nickname: string;
    emailOrLogin: string;
  };

  type MainTab = "home" | "skins" | "modes";
  type ViewMode = "home" | "account" | "launcher-settings";
  type ThemeMode = "dark" | "light";
  type BuildItem = {
    id: string;
    name: string;
    description: string;
    imageFile: string;
    installed?: boolean;
    filters?: string[];
    loader?: string;
    gameVersion?: string;
  };

  type BuildFilterDefinition = {
    id: string;
    label: string;
  };

  type OpenModeRequest = {
    id: number;
    buildId: string;
  };

  export let user: SessionUser;
  const dispatch = createEventDispatcher<{ signOut: void }>();

  const themeStorageKey = "tbwlauncher-theme";
  const recentModesStorageKey = "tbwlauncher-recent-modes";

  let activeTab: MainTab = "home";
  let viewMode: ViewMode = "home";
  let theme: ThemeMode = "dark";
  let skinPreviewUrl = "";
  let showSignOutConfirm = false;
  let runningModeName: string | null = null;
  let openModeRequest: OpenModeRequest | null = null;
  let openModeRequestIdCounter = 0;

  let recentModes: string[] = [];

  let buildNames: string[] = [
    "ARCADE",
    "Laboratory",
    "CROSSBATTLES",
    "INFECTED",
    "MAFIA: Last Visitor",
    'Operation "Blizzard"',
    "SLASHER: Redwood",
    "SCAVENGER",
    "SHADOWS",
    "PHASMOPHOBIA: Halloween",
    "RAKE",
    "Counter Craft",
  ];

  let buildFilterDefinitions: BuildFilterDefinition[] = [
    { id: "rp", label: "RolePlay" },
    { id: "horror", label: "Хоррор" },
    { id: "shooter", label: "Шутер" },
  ];

  let buildDescriptionsByName: Record<string, string> = {
    ARCADE:
      "▪︎ Шутер подставляющий собой сборник карт и поджанров таких как DeathMatch и т. д. \n▪︎ Игроки появляются на карте с случайным оружием в руках. \n▪︎ Цель команд убить как можно больше других игроков, при достижении нужного количества убийств, команда побеждает.",
    Laboratory:
      '▪︎ В комплексе "D7" секретной организации "Netherhill" произошло нарушение условий содержания. \n▪︎ Ваша задача взять на себя роль "Специалиста" и сбежать из комплекса.\n▪︎ Либо вооружившись винтовкой и бронёй "CSG" пополнить ряды Оперативников, дабы выполнить свою миссию: "Устранить Специалистов, опасного Монстра и нормализовать работу Реактора".\n▪︎ Также есть возможность стать опасным объектом "NH-67B", который может разорвать всех в клочья',
  };

  let buildImagesByName: Record<string, string> = {
    ARCADE: "ARCADE.png",
    Laboratory: "Laboratory.png",
    CROSSBATTLES: "CROSSBATTLES.png",
    INFECTED: "INFECTED.png",
    "MAFIA: Last Visitor": "MAFIA.png",
    'Operation "Blizzard"': "Operation Blizzard.png",
    "SLASHER: Redwood": "SLASHER Redwood.png",
    SCAVENGER: "SCAVENGER.png",
    SHADOWS: "SHADOWS.png",
    "PHASMOPHOBIA: Halloween": "PHASMOPHOBIA.png",
    RAKE: "RAKE.png",
    "Counter Craft": "Counter Craft.png",
  };

  let buildMetaByName: Record<
    string,
    {
      installed?: boolean;
      filters?: string[];
      loader?: string;
      gameVersion?: string;
    }
  > = {
    ARCADE: {
      installed: true,
      filters: ["shooter"],
      loader: "Forge",
      gameVersion: "1.20.1",
    },
    Laboratory: {
      installed: true,
      filters: ["rp"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
    CROSSBATTLES: {
      installed: true,
      filters: ["shooter"],
      loader: "Forge",
      gameVersion: "1.20.1",
    },
    INFECTED: {
      installed: false,
      filters: ["horror"],
      loader: "Fabric",
      gameVersion: "1.12.2",
    },
    "MAFIA: Last Visitor": {
      installed: false,
      filters: ["rp"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
    'Operation "Blizzard"': {
      installed: false,
      filters: ["shooter"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
    "SLASHER: Redwood": {
      installed: false,
      filters: ["horror"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
    SCAVENGER: {
      installed: false,
      filters: ["horror"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
    SHADOWS: {
      installed: false,
      filters: ["horror"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
    "PHASMOPHOBIA: Halloween": {
      installed: false,
      filters: ["horror"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
    RAKE: {
      installed: false,
      filters: ["horror"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
    "Counter Craft": {
      installed: false,
      filters: ["shooter"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
  };

  let assetImageNames: string[] = Object.values(buildImagesByName);
  const bundledAssetUrlsByName = Object.fromEntries(
    Object.entries(
      import.meta.glob("/src/assets/*.{png,jpg,jpeg,webp}", {
        eager: true,
        import: "default",
      }) as Record<string, string>,
    ).map(([fullPath, url]) => {
      const fileName = fullPath.split("/").pop() ?? fullPath;
      return [fileName, url];
    }),
  );

  $: builds = buildNames.map((name) => {
    const meta = buildMetaByName[name] ?? {};

    return {
      id: makeBuildId(name),
      name,
      description: buildDescriptionsByName[name] ?? "Описание не задано.",
      imageFile: buildImagesByName[name] ?? "",
      installed: meta.installed ?? false,
      filters: meta.filters ?? [],
      loader: meta.loader ?? "Forge",
      gameVersion: meta.gameVersion ?? "1.20.1",
    };
  }) as BuildItem[];
  $: recentBuilds = recentModes
    .map((modeName) => builds.find((build) => build.name === modeName))
    .filter(Boolean) as BuildItem[];

  $: accountInitials = user.nickname.trim().slice(0, 1).toUpperCase() || "P";

  onMount(() => {
    const storedTheme = localStorage.getItem(themeStorageKey);
    if (storedTheme === "dark" || storedTheme === "light") {
      theme = storedTheme;
    }

    const rawRecentModes = localStorage.getItem(recentModesStorageKey);
    if (rawRecentModes) {
      try {
        const parsed = JSON.parse(rawRecentModes) as string[];
        if (Array.isArray(parsed)) {
          recentModes = parsed.slice(0, 3);
        }
      } catch {
        recentModes = [];
      }
    }

    applyTheme(theme);
  });

  function openAccountSettings(): void {
    viewMode = "account";
  }

  function openLauncherSettings(): void {
    viewMode = "launcher-settings";
  }

  function returnHome(): void {
    viewMode = "home";
  }

  function updateTheme(nextTheme: ThemeMode): void {
    theme = nextTheme;
    applyTheme(nextTheme);
    localStorage.setItem(themeStorageKey, nextTheme);
  }

  function saveNickname(nextNickname: string): void {
    user = {
      ...user,
      nickname: nextNickname,
    };
  }

  function applyTheme(nextTheme: ThemeMode): void {
    document.documentElement.dataset.theme = nextTheme;
  }

  function makeBuildId(name: string): string {
    return name.toLowerCase().replace(/[^a-zа-яё0-9]/gi, "-");
  }

  function handleModeLaunch(event: CustomEvent<{ modeName: string }>): void {
    toggleModeRuntime(event.detail.modeName);
  }

  function launchRecentMode(modeName: string): void {
    toggleModeRuntime(modeName);
  }

  function openModeFromRecent(modeName: string): void {
    activeTab = "modes";
    openModeRequestIdCounter += 1;
    openModeRequest = {
      id: openModeRequestIdCounter,
      buildId: makeBuildId(modeName),
    };
  }

  function toggleModeRuntime(modeName: string): void {
    if (runningModeName === modeName) {
      runningModeName = null;
      return;
    }

    runningModeName = modeName;
    markModeLaunched(modeName);
  }

  function markModeLaunched(modeName: string): void {
    recentModes = [
      modeName,
      ...recentModes.filter((item) => item !== modeName),
    ].slice(0, 3);
    localStorage.setItem(recentModesStorageKey, JSON.stringify(recentModes));
  }

  function resolveAssetUrl(fileName: string): string {
    const bundledUrl = bundledAssetUrlsByName[fileName];
    if (bundledUrl) {
      return bundledUrl;
    }

    return `/assets/${fileName}`;
  }

  function requestSignOut(): void {
    showSignOutConfirm = true;
  }

  function cancelSignOut(): void {
    showSignOutConfirm = false;
  }

  function signOut(): void {
    showSignOutConfirm = false;
    runningModeName = null;
    openModeRequest = null;
    openModeRequestIdCounter = 0;
    dispatch("signOut");
  }
</script>

<main class="launcher-root">
  <section class="launcher-shell">
    <aside class="left-nav">
      <div class="nav-buttons">
        <button
          type="button"
          class="nav-button"
          class:active={activeTab === "home"}
          on:click={() => (activeTab = "home")}
          aria-label="Домашняя страница"
          title="Домашняя страница"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M3 10.5L12 3l9 7.5" />
            <path d="M6 9.5V21h12V9.5" />
          </svg>
        </button>

        <button
          type="button"
          class="nav-button"
          class:active={activeTab === "skins"}
          on:click={() => (activeTab = "skins")}
          aria-label="Скины"
          title="Скины"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 7V5a2 2 0 1 0-2-2" />
            <path d="M3.5 12.5L12 7l8.5 5.5" />
            <path d="M6.5 12.5l2.5 6h6l2.5-6" />
          </svg>
        </button>

        <button
          type="button"
          class="nav-button"
          class:active={activeTab === "modes"}
          on:click={() => (activeTab = "modes")}
          aria-label="Режимы"
          title="Режимы"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <rect x="4" y="5" width="16" height="4" rx="1" />
            <rect x="4" y="10" width="16" height="4" rx="1" />
            <rect x="4" y="15" width="16" height="4" rx="1" />
          </svg>
        </button>
      </div>

      <button
        class="nav-exit"
        type="button"
        aria-label="Выход"
        title="Выход"
        on:click={requestSignOut}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M10 4h7a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-7" />
          <path d="M15 12H4" />
          <path d="M8 8l-4 4 4 4" />
        </svg>
      </button>
    </aside>

    <section class="launcher-card">
      <header class="launcher-head">
        <h1>
          {activeTab === "home"
            ? "Домашняя страница"
            : activeTab === "skins"
              ? "Скины"
              : "Режимы"}
        </h1>

        <button
          class="profile-chip"
          type="button"
          on:click={openAccountSettings}
          aria-label="Открыть настройки аккаунта"
        >
          <div class="skin-face" aria-hidden="true">
            {#if skinPreviewUrl}
              <img src={skinPreviewUrl} alt="Лицо скина" />
            {:else}
              {accountInitials}
            {/if}
          </div>
          <span>{user.nickname}</span>
        </button>
      </header>

      <section class="home-layer" class:blurred={viewMode !== "home"}>
        {#key activeTab}
          <div
            class="tab-scene"
            in:fly={{ x: 18, duration: 220, easing: cubicOut }}
            out:fade={{ duration: 120 }}
          >
            {#if activeTab === "home"}
              <section class="tab-placeholder recent-panel">
                <h2>Недавно запущенные режимы</h2>
                {#if recentModes.length === 0}
                  <p>Вы еще не запускали ни одной сборки</p>
                {:else}
                  <div class="recent-list">
                    {#each recentBuilds as mode (mode.id)}
                      <div
                        class="recent-item"
                        role="button"
                        tabindex="0"
                        aria-label={`Открыть режим ${mode.name}`}
                        on:click={() => openModeFromRecent(mode.name)}
                        on:keydown={(event) => {
                          if (event.key === "Enter" || event.key === " ") {
                            event.preventDefault();
                            openModeFromRecent(mode.name);
                          }
                        }}
                      >
                        <div class="recent-thumb-wrap">
                          <img
                            src={resolveAssetUrl(mode.imageFile)}
                            alt={`Изображение режима ${mode.name}`}
                          />
                        </div>
                        <h3>{mode.name}</h3>
                        <div class="recent-meta-row">
                          <p>
                            {mode.loader ?? "Forge"}
                            {mode.gameVersion ?? "1.20.1"}
                          </p>
                          <button
                            type="button"
                            class="recent-launch-btn"
                            class:running={runningModeName === mode.name}
                            on:click|stopPropagation={() => launchRecentMode(mode.name)}
                          >
                            {#if runningModeName === mode.name}
                              Закрыть игру
                            {:else}
                              Запустить
                            {/if}
                          </button>
                        </div>
                      </div>
                    {/each}
                  </div>
                {/if}
                <div class="recent-reserved-space" aria-hidden="true"></div>
              </section>
            {:else if activeTab === "skins"}
              <section class="tab-placeholder">
                <h2>Скины</h2>
                <p>Говно Говно Говно Говно Говно Говно</p>
              </section>
            {:else}
              <BuildShowcase
                {builds}
                filterDefinitions={buildFilterDefinitions}
                {assetImageNames}
                {bundledAssetUrlsByName}
                {runningModeName}
                {openModeRequest}
                on:launch={handleModeLaunch}
              />
            {/if}
          </div>
        {/key}
      </section>
    </section>
  </section>

  {#if viewMode !== "home"}
    <div
      class="overlay-backdrop"
      role="presentation"
      in:fade={{ duration: 150 }}
      out:fade={{ duration: 120 }}
    >
      <button
        type="button"
        class="overlay-dismiss"
        on:click={returnHome}
        aria-label="Закрыть окно настроек"
      ></button>
      <section
        class="overlay-panel"
        role="dialog"
        aria-modal="true"
        in:scale={{ start: 0.94, duration: 180, easing: cubicOut }}
        out:fade={{ duration: 120 }}
      >
        {#if viewMode === "account"}
          <AccountSettingsPanel
            account={user}
            onBack={returnHome}
            onNicknameSave={saveNickname}
          />
        {:else}
          <LauncherSettingsPanel
            {theme}
            onBack={returnHome}
            onThemeChange={updateTheme}
          />
        {/if}
      </section>
    </div>
  {/if}

  {#if showSignOutConfirm}
    <div
      class="confirm-backdrop"
      role="presentation"
      in:fade={{ duration: 120 }}
      out:fade={{ duration: 100 }}
    >
      <div
        class="confirm-panel"
        role="dialog"
        aria-modal="true"
        aria-label="Подтверждение выхода"
        in:fly={{ y: 20, duration: 180, easing: cubicOut }}
        out:fade={{ duration: 120 }}
      >
        <p>Вы действительно хотите выйти из аккаунта?</p>
        <div class="confirm-actions">
          <button type="button" class="confirm-cancel" on:click={cancelSignOut}
            >Отмена</button
          >
          <button type="button" class="confirm-submit" on:click={signOut}
            >Подтвердить</button
          >
        </div>
      </div>
    </div>
  {/if}

  {#if viewMode === "home"}
    <button
      type="button"
      class="launcher-settings-btn"
      on:click={openLauncherSettings}
      aria-label="Открыть настройки лаунчера"
      title="Настройки лаунчера"
    >
      ⚙
    </button>
  {/if}
</main>

<style>
  .launcher-root {
    min-height: 100vh;
    position: relative;
    display: block;
    padding: 0;
    background: var(--background-main);
  }

  .launcher-shell {
    min-height: 100vh;
    display: grid;
    grid-template-columns: 92px 1fr;
    gap: 12px;
    padding: 12px;
  }

  .left-nav {
    border: 1px solid var(--line);
    background: var(--surface-alt);
    border-radius: 16px;
    padding: 10px;
    display: grid;
    grid-template-rows: auto 1fr auto;
    height: calc(100vh - 24px);
    min-height: 0;
    position: sticky;
    top: 12px;
    align-self: start;
    padding-bottom: 68px;
  }

  .nav-buttons {
    display: grid;
    align-content: start;
    gap: 8px;
  }

  .nav-button {
    width: 90%;
    justify-self: center;
    height: 52px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--surface-main);
    color: var(--text-main);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition:
      transform 0.16s ease,
      border-color 0.16s ease,
      box-shadow 0.16s ease,
      background-color 0.16s ease;
  }

  .nav-button svg {
    width: 22px;
    height: 22px;
    stroke: currentColor;
    fill: none;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .nav-button.active {
    border-color: var(--accent);
    background: var(--surface-elevated);
  }

  .nav-button:hover {
    transform: scale(1.045);
    border-color: var(--accent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 42%, transparent);
  }

  .nav-button:active {
    transform: scale(0.98);
  }

  .nav-exit {
    position: absolute;
    left: 50%;
    bottom: 10px;
    transform: translateX(-50%);
    width: 48px;
    height: 48px;
    border-radius: 12px;
    border: 1px solid rgba(255, 82, 82, 0.55);
    background: rgba(255, 56, 56, 0.12);
    color: #ff5d5d;
    display: inline-grid;
    place-items: center;
    cursor: pointer;
    transition:
      transform 0.16s ease,
      border-color 0.16s ease,
      box-shadow 0.16s ease,
      background-color 0.16s ease;
  }

  .nav-exit:hover {
    transform: translateX(-50%) scale(1.045);
    border-color: rgba(255, 96, 96, 0.9);
    box-shadow: 0 0 0 1px rgba(255, 82, 82, 0.35);
    background: rgba(255, 56, 56, 0.2);
  }

  .nav-exit:active {
    transform: translateX(-50%) scale(0.96);
  }

  button:not(.overlay-dismiss):hover {
    filter: brightness(1.07);
  }

  button:not(.overlay-dismiss):active {
    animation: button-press-glow 0.2s ease;
  }

  @keyframes button-press-glow {
    0% {
      filter: brightness(1);
    }
    50% {
      filter: brightness(1.16);
    }
    100% {
      filter: brightness(1);
    }
  }

  .nav-exit svg {
    width: 24px;
    height: 24px;
    stroke: currentColor;
    fill: none;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .launcher-card {
    width: 100%;
    height: calc(100vh - 24px);
    min-height: 0;
    border: none;
    border-radius: 16px;
    background: var(--surface-main);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-main);
    padding: clamp(12px, 1.2vw, 16px);
    display: grid;
    grid-template-rows: auto 1fr;
    gap: 10px;
    overflow: hidden;
  }

  .launcher-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  h1 {
    margin: 0;
    font-size: clamp(1.5rem, 2.2vw, 2.1rem);
  }

  .profile-chip {
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    border-radius: 14px;
    padding: 6px 10px 6px 6px;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    font: inherit;
    transition: transform 0.16s ease;
  }

  .profile-chip:hover {
    transform: scale(1.04);
    filter: none;
  }

  .profile-chip:active {
    transform: scale(0.98);
  }

  .skin-face {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, #3cbcad, #5c86f6);
    color: #f1fbff;
    font-weight: 700;
  }

  .skin-face img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .launcher-settings-btn {
    position: fixed;
    right: 16px;
    bottom: 16px;
    width: 52px;
    height: 52px;
    border-radius: 14px;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    font-size: 1.5rem;
    cursor: pointer;
    z-index: 2;
    transition:
      transform 0.16s ease,
      border-color 0.16s ease,
      box-shadow 0.16s ease,
      background-color 0.16s ease;
  }

  .launcher-settings-btn:hover {
    transform: scale(1.06);
    border-color: var(--accent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent);
    background: var(--surface-elevated);
  }

  .launcher-settings-btn:active {
    transform: scale(0.96);
  }

  .home-layer {
    min-height: 0;
    overflow: hidden;
    transition:
      filter 0.2s ease,
      transform 0.2s ease,
      opacity 0.2s ease;
  }

  .tab-scene {
    min-height: 0;
    height: 100%;
  }

  .home-layer.blurred {
    filter: blur(7px);
    transform: scale(0.992);
    opacity: 0.8;
    pointer-events: none;
    user-select: none;
  }

  .tab-placeholder {
    border: 1px solid var(--line);
    background: var(--surface-alt);
    border-radius: 12px;
    min-height: 380px;
    padding: 16px;
  }

  .recent-panel {
    min-height: 0;
    padding-top: 10px;
    padding-bottom: 5px;
  }

  .tab-placeholder h2 {
    margin: 0 0 0;
  }

  .recent-panel h2 {
    margin-top: -2px;
  }

  .tab-placeholder p {
    margin: 0;
    color: var(--text-muted);
  }

  .recent-list {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    margin-top: 6px;
  }

  .recent-item {
    border: 1px solid var(--line);
    background: var(--surface-main);
    border-radius: 10px;
    padding: 7px 7px 6px;
    color: var(--text-main);
    display: grid;
    gap: 2px;
    min-width: 0;
    cursor: pointer;
    transition:
      transform 0.16s ease,
      border-color 0.16s ease,
      box-shadow 0.16s ease,
      background-color 0.16s ease;
  }

  .recent-item:hover {
    transform: scale(1.02);
    border-color: var(--accent);
    background: var(--surface-elevated);
    box-shadow: 0 10px 20px rgba(9, 16, 26, 0.24);
  }

  .recent-item:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 72%, transparent);
    outline-offset: 2px;
  }

  .recent-item:active {
    transform: scale(0.99);
  }

  .recent-thumb-wrap {
    width: 100%;
    aspect-ratio: 16 / 9;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid var(--line);
    background: var(--surface-alt);
  }

  .recent-thumb-wrap img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .recent-item h3 {
    margin: 0;
    margin-top: 6px;
    margin-bottom: -1px;
    font-size: 0.82rem;
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .recent-meta-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    margin-top: 0;
  }

  .recent-item p {
    margin: 0;
    margin-top: 2px;
    font-size: 0.74rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .recent-launch-btn {
    flex: 0 0 auto;
    width: 124px;
    display: inline-flex;
    justify-content: center;
    align-items: center;
    border: none;
    border-radius: 7px;
    padding: 5px 8px;
    font: inherit;
    font-size: 0.74rem;
    font-weight: 700;
    color: #0b1422;
    background: linear-gradient(135deg, #9ef7ce, #84bffb);
    cursor: pointer;
    transition:
      transform 0.14s ease,
      box-shadow 0.14s ease,
      filter 0.14s ease;
  }

  .recent-launch-btn:hover {
    transform: translateY(-1px);
    box-shadow: 0 3px 10px rgba(72, 191, 180, 0.28);
    filter: brightness(1.05);
  }

  .recent-launch-btn.running {
    color: #ffeaea;
    background: linear-gradient(135deg, #eb5d5d, #b92020);
    box-shadow: 0 4px 12px rgba(168, 38, 38, 0.28);
  }

  .recent-launch-btn.running:hover {
    box-shadow: 0 4px 12px rgba(168, 38, 38, 0.38);
  }

  .recent-reserved-space {
    min-height: 0;
  }

  .overlay-backdrop {
    position: fixed;
    inset: 0;
    z-index: 3;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(6, 10, 16, 0.5);
    backdrop-filter: blur(6px);
  }

  .overlay-dismiss {
    position: absolute;
    inset: 0;
    border: none;
    background: transparent;
    cursor: default;
  }

  .overlay-panel {
    position: relative;
    width: min(820px, 96vw);
    max-height: calc(100vh - 48px);
    overflow: auto;
    border-radius: 16px;
    border: 1px solid var(--line);
    background: var(--surface-main);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
    padding: 16px;
  }

  .confirm-backdrop {
    position: fixed;
    inset: 0;
    z-index: 6;
    background: rgba(5, 8, 14, 0.55);
    backdrop-filter: blur(3px);
    display: grid;
    place-items: center;
    padding: 18px;
  }

  .confirm-panel {
    width: min(420px, 92vw);
    border: 1px solid var(--line);
    background: var(--surface-main);
    border-radius: 14px;
    padding: 16px;
    box-shadow: 0 18px 45px rgba(0, 0, 0, 0.35);
    display: grid;
    gap: 14px;
    transform-origin: left bottom;
    animation: signout-pop-in 0.22s ease-out;
  }

  @keyframes signout-pop-in {
    from {
      opacity: 0;
      transform: translate(-18px, 18px) scale(0.93);
    }
    to {
      opacity: 1;
      transform: translate(0, 0) scale(1);
    }
  }

  .confirm-panel p {
    margin: 0;
    color: var(--text-main);
    font-size: 1rem;
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  .confirm-cancel,
  .confirm-submit {
    border-radius: 10px;
    padding: 8px 12px;
    font: inherit;
    cursor: pointer;
    transition:
      transform 0.14s ease,
      border-color 0.14s ease,
      box-shadow 0.14s ease,
      background-color 0.14s ease;
  }

  .confirm-cancel {
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
  }

  .confirm-submit {
    border: 1px solid rgba(255, 82, 82, 0.55);
    background: rgba(255, 56, 56, 0.16);
    color: #ff7070;
  }

  .confirm-cancel:hover {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 36%, transparent);
    transform: translateY(-1px);
  }

  .confirm-submit:hover {
    border-color: rgba(255, 100, 100, 0.92);
    box-shadow: 0 0 0 1px rgba(255, 82, 82, 0.32);
    background: rgba(255, 56, 56, 0.24);
    transform: translateY(-1px);
  }

  @media (max-width: 700px) {
    .launcher-shell {
      grid-template-columns: 1fr;
      gap: 10px;
    }

    .left-nav {
      min-height: auto;
      position: static;
      grid-template-rows: auto auto auto;
    }

    .launcher-head {
      align-items: flex-start;
      flex-direction: column;
    }

    .profile-chip {
      align-self: flex-end;
    }
  }
</style>
