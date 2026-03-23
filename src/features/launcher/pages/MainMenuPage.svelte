<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly, scale } from "svelte/transition";
  import BuildShowcase from "../components/BuildShowcase.svelte";
  import SkinStudio from "../components/SkinStudio.svelte";
  import AccountSettingsPanel from "../components/AccountSettingsPanel.svelte";
  import LauncherSettingsPanel from "../components/LauncherSettingsPanel.svelte";
  import AdminPanel from "../components/AdminPanel.svelte";
  import { HOME_PROMO_CARDS, type HomePromoCard } from "../models/home-content";
  import {
    checkLauncherUpdate as checkLauncherUpdateCommand,
    downloadAndInstallLauncherUpdate as downloadAndInstallLauncherUpdateCommand,
    restartLauncher as restartLauncherCommand,
    type LauncherUpdateDownloadProgress,
    type LauncherUpdateHandle,
  } from "../services/launcher-updater-service";
  import {
    cancelActiveDownloads as cancelActiveDownloadsCommand,
    type BuildInstallProgressState,
    type BuildInstallationState,
    getBuildInstallationStates as getBuildInstallationStatesCommand,
    getGameRuntimeState as getGameRuntimeStateCommand,
    getInstallProgressState as getInstallProgressStateCommand,
    installBuild as installBuildCommand,
    toggleGameRuntime as toggleGameRuntimeCommand,
    updateDiscordPresence as updateDiscordPresenceCommand,
  } from "../services/game-runtime-service";
  import { openExternalUrl as openExternalUrlCommand } from "../services/external-link-service";
  import {
    attachHoverSounds,
    playLaunchSound,
    playPanelCloseSound,
    playPanelOpenSound,
    playSwitchSound,
  } from "../services/ui-sound";
  import {
    changePassword,
    getAccountChangeStatus,
    updateAccount,
    updateStoredSessionNickname,
  } from "../../auth/services/auth-service";
  import type { AccountChangeStatus } from "../../auth/models/auth";
  import { loadLauncherSettings as loadLauncherSettingsCommand } from "../services/launcher-settings-service";

  type SessionUser = {
    id: string;
    nickname: string;
    emailOrLogin: string;
    skinUrl?: string;
    role: string;
  };

  type MainTab = "home" | "skins" | "modes";
  type ViewMode = "home" | "account" | "launcher-settings" | "admin";
  type ThemeMode = "dark" | "light";
  type BuildItem = {
    id: string;
    name: string;
    description: string;
    imageFile: string;
    installed?: boolean;
    updateAvailable?: boolean;
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

  type AccountSavePayload = {
    nickname: string;
    currentPassword: string;
    nextPassword: string;
  };

  export let user: SessionUser;
  const dispatch = createEventDispatcher<{ signOut: void }>();

  const themeStorageKey = "tbwlauncher-theme";
  const recentModesStorageKey = "tbwlauncher-recent-modes";
  const skinPreviewStorageKey = "tbwlauncher-skin-preview";
  const launcherUpdateInstalledToastStorageKey =
    "tbwlauncher-update-installed-toast";
  const launcherUpdateInstalledToastDurationMs = 5000;
  const accountStatusRequestTimeoutMs = 8000;
  const downloadCancelledErrorCode = "TBW_OPERATION_CANCELLED";
  const updateCheckDebounceMs = 1800;
  const autoUpdatePeriodicCheckIntervalMs = 10 * 60 * 1000;
  const maxHomeRecentRows = 3;
  const homePromoCards: HomePromoCard[] = HOME_PROMO_CARDS;

  let activeTab: MainTab = "home";
  let viewMode: ViewMode = "home";
  let theme: ThemeMode = "dark";
  let skinPreviewUrl = "";
  let selectedSkinUrl = "";
  let showSignOutConfirm = false;
  let showPromoNavigationConfirm = false;
  let promoNavigationUrl = "";
  let runningModeName: string | null = null;
  let openModeRequest: OpenModeRequest | null = null;
  let openModeRequestIdCounter = 0;
  let launcherRootElement: HTMLElement;
  let launchInFlight = false;
  let launchCancelRequested = false;
  let launchPendingModeName: string | null = null;
  let showLaunchProgress = false;
  let launchProgress = 0;
  let launchStatusText = "";
  let launchProgressTimer: ReturnType<typeof setInterval> | null = null;
  let installProgressPollTimer: ReturnType<typeof setInterval> | null = null;
  let launchProgressHideTimer: ReturnType<typeof setTimeout> | null = null;
  let usingRealLaunchProgress = false;
  let runtimeStatePollTimer: ReturnType<typeof setInterval> | null = null;
  let installedBuildStateByName: Record<string, BuildInstallationState> = {};
  let discordPresenceReady = false;
  let lastDiscordPresenceModeName: string | null | undefined = undefined;
  let lastDiscordPresenceNickname: string | undefined = undefined;
  let accountChangeStatus: AccountChangeStatus | null = null;
  let accountChangeStatusLoading = false;
  let accountChangeStatusRequestId = 0;
  let updateCheckInFlight = false;
  let updateInstallInFlight = false;
  let showLauncherUpdatePrompt = false;
  let launcherUpdateBody = "";
  let launcherUpdateVersion = "";
  let launcherCurrentVersion = "";
  let launcherUpdateError = "";
  let launcherUpdateDownloadedBytes = 0;
  let launcherUpdateTotalBytes: number | null = null;
  let launcherUpdatePercent: number | null = null;
  let launcherAutoUpdatesEnabled = true;
  let launcherUpdateHandle: LauncherUpdateHandle | null = null;
  let scheduledAutoUpdateCheckTimer: ReturnType<typeof setTimeout> | null =
    null;
  let periodicAutoUpdateCheckTimer: ReturnType<typeof setInterval> | null =
    null;
  let showLauncherUpdateInstalledToast = false;
  let launcherUpdateInstalledToastText = "Обновление лаунчера установлено.";
  let launcherUpdateInstalledToastTimer: ReturnType<typeof setTimeout> | null =
    null;

  let recentModes: string[] = [];
  let homeRecentBuilds: BuildItem[] = [];

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
    CROSSBATTLES:
      "▪︎ На карте вам предстоит сражаться с другими командами, покупая оружие на накопанную вaми руду.",
    INFECTED:
      "▪ INFECTED - Это психологическая игра, похожая на Мафию (или же Deceit, Among Us).\n▪ Выжившие должны выполнять задания и стараться понять, кто является монстром.\n▪ В тот момент как заражённый должен всячески этому препятствовать.",
    "MAFIA: Last Visitor":
      "▪︎ Шторм загнал путников в старый отель. Среди гостей затесалась мафия — но кто он?. \n▪︎ Мирные (жители, доктор, шериф) ищут предателей. \n▪︎ Мафия устраняет всех. Чтобы победить, используй тактики и изучай психологию человека. \n▪︎ Днем — споры и голосования. \n▪︎ Ночью — тайные убийства. Кто выживет?",
    'Operation "Blizzard"':
      '▪︎ Есть две команды: "Атака" и "Оборона".\n▪︎ "Атаке" нужно захватить и принести себе на базу секретные документы, спрятанные в горном исследовательском комплексе.\n▪︎ "Обороне" Нужно тщательно этому помешать.',
    "SLASHER: Redwood":
      "▪︎ На этой карте вам предстоит взять на себя роль выжившего, либо кровожадного монстра.\n▪︎ Играя за чувырлу - придётся выслеживать и убивать людей, мешая им совершить побег.\n▪︎ Играя за выжившего нужно найти способ побега, выполняя различные задания.",
    SCAVENGER:
      "▪︎ На этой карте вам предстоит взять на себя роль выжившего, либо кровожадного монстра.\n▪︎ Играя за монстра - придётся выслеживать и убивать людей, мешая им совершить побег.\n▪︎ Играя за выжившего вам нужно найти все четыре трупа, взять био-материал монстра и разработать сыворотку, с помощью которой вы и убьёте существо.",
    SHADOWS:
      '▪ Игровой режим на подобии "Slasher:Redwood" или же "Scavenger".\n▪ Цель егерей сначала сфотографировать монстра, а позже зафиксировать его "активность" с помощью Детектора ЭМП.\n▪ Целью монстра является выживание и убийство всех егерей.',
    "PHASMOPHOBIA: Halloween":
      "▪︎ Задача Охотников: понять, к какому типу относится Призрак, после чего найти и принести в фургон Реликвию.\n▪︎ Задача Призрака: Убить всех Охотников и не позволить вынести реликвию из Дома.",
    RAKE: '▪︎ Вы, исследовательская группа которая приехала в лес, находящийся в аномальном районе, в округе "Redwood", и заметили странную активность в этом лесу.\n▪︎ Возьмите на себя роль храброго охотника, который должен неизвестное существо.\n▪︎ Монстра, который должен сломать два генератора, что бы лишить егерей поставок ресурсов.',
    "Counter Craft": "▪︎ Counter Strike: Global Offencive в Minecraft.",
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
      installed: false,
      filters: ["shooter"],
      loader: "Forge",
      gameVersion: "1.20.1",
    },
    Laboratory: {
      installed: false,
      filters: ["rp"],
      loader: "Forge",
      gameVersion: "1.12.2",
    },
    CROSSBATTLES: {
      installed: false,
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
    const installationState = installedBuildStateByName[name];

    return {
      id: makeBuildId(name),
      name,
      description: buildDescriptionsByName[name] ?? "Описание не задано.",
      imageFile: buildImagesByName[name] ?? "",
      installed: installationState?.installed ?? false,
      updateAvailable: installationState?.updateAvailable ?? false,
      filters: meta.filters ?? [],
      loader: meta.loader ?? "Forge",
      gameVersion: meta.gameVersion ?? "1.20.1",
    };
  }) as BuildItem[];
  $: recentBuilds = recentModes
    .map((modeName) => builds.find((build) => build.name === modeName))
    .filter(Boolean) as BuildItem[];
  $: homeRecentBuilds = (() => {
    const uniqueBuilds: BuildItem[] = [];
    const seenBuildIds = new Set<string>();

    for (const build of recentBuilds) {
      if (seenBuildIds.has(build.id)) {
        continue;
      }

      seenBuildIds.add(build.id);
      uniqueBuilds.push(build);

      if (uniqueBuilds.length >= maxHomeRecentRows) {
        break;
      }
    }

    return uniqueBuilds;
  })();
  $: if (!selectedSkinUrl && user.skinUrl?.trim()) {
    selectedSkinUrl = user.skinUrl.trim();
  }

  $: accountInitials = user.nickname.trim().slice(0, 1).toUpperCase() || "P";
  $: canAccessAdminPanel =
    user.role.trim().toLowerCase() === "admin" ||
    user.role.trim().toLowerCase() === "tech";

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
          recentModes = parsed.slice(0, maxHomeRecentRows);
        }
      } catch {
        recentModes = [];
      }
    }

    const storedSkinPreview = localStorage.getItem(skinPreviewStorageKey);
    if (storedSkinPreview) {
      skinPreviewUrl = storedSkinPreview;
    }
    selectedSkinUrl = user.skinUrl?.trim() ?? "";

    applyTheme(theme);
    consumeInstalledUpdateToastMarker();
    void refreshRuntimeState();
    void refreshBuildInstallationStates();
    const currentNickname = user.nickname.trim();
    discordPresenceReady = true;
    lastDiscordPresenceModeName = runningModeName;
    lastDiscordPresenceNickname = currentNickname;
    void syncDiscordPresence(runningModeName, currentNickname);
    void initializeLauncherAutoUpdateState();

    const detachHoverSounds = launcherRootElement
      ? attachHoverSounds(launcherRootElement)
      : () => {};
    const handleWindowFocus = () => {
      void refreshRuntimeState();
      void refreshBuildInstallationStates();
      void syncDiscordPresence(runningModeName, user.nickname.trim());
    };
    const handleWindowKeydown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      if (showSignOutConfirm) {
        event.preventDefault();
        cancelSignOut();
        return;
      }

      if (showPromoNavigationConfirm) {
        event.preventDefault();
        dismissPromoNavigationConfirm();
        return;
      }

      if (viewMode === "account" || viewMode === "launcher-settings") {
        event.preventDefault();
        returnHome();
      }
    };

    if (typeof window !== "undefined") {
      window.addEventListener("focus", handleWindowFocus);
      window.addEventListener("keydown", handleWindowKeydown);
    }

    runtimeStatePollTimer = setInterval(() => {
      void refreshRuntimeState();
      void refreshBuildInstallationStates();
    }, 2000);

    return () => {
      if (runtimeStatePollTimer) {
        clearInterval(runtimeStatePollTimer);
        runtimeStatePollTimer = null;
      }

      if (typeof window !== "undefined") {
        window.removeEventListener("focus", handleWindowFocus);
        window.removeEventListener("keydown", handleWindowKeydown);
      }

      detachHoverSounds();
      clearLaunchProgressTimers();
      clearLauncherAutoUpdateSchedule();
      clearLauncherUpdateInstalledToastTimer();
    };
  });

  function withTimeout<T>(
    promise: Promise<T>,
    timeoutMs: number,
    errorMessage: string,
  ): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const timeoutHandle = setTimeout(() => {
        reject(new Error(errorMessage));
      }, timeoutMs);

      promise.then(
        (value) => {
          clearTimeout(timeoutHandle);
          resolve(value);
        },
        (error) => {
          clearTimeout(timeoutHandle);
          reject(error);
        },
      );
    });
  }

  function formatCompactBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes < 0) {
      return "0 B";
    }

    if (bytes >= 1024 * 1024 * 1024) {
      return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
    }

    if (bytes >= 1024 * 1024) {
      return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    }

    if (bytes >= 1024) {
      return `${(bytes / 1024).toFixed(1)} KB`;
    }

    return `${Math.round(bytes)} B`;
  }

  function formatUpdaterError(error: unknown): string {
    const fallback = "Не удалось установить обновление лаунчера.";

    if (typeof error === "string" && error.trim()) {
      return error.trim();
    }

    if (error instanceof Error && error.message.trim()) {
      return error.message.trim();
    }

    try {
      const serialized = JSON.stringify(error);
      if (serialized && serialized !== "{}") {
        return serialized;
      }
    } catch {}

    return fallback;
  }

  function resetLauncherUpdateProgress(): void {
    launcherUpdateDownloadedBytes = 0;
    launcherUpdateTotalBytes = null;
    launcherUpdatePercent = null;
  }

  function handleLauncherUpdaterProgress(
    progress: LauncherUpdateDownloadProgress,
  ): void {
    launcherUpdateDownloadedBytes = progress.downloadedBytes;
    launcherUpdateTotalBytes = progress.totalBytes;
    launcherUpdatePercent = progress.percent;
  }

  function clearLauncherUpdateInstalledToastTimer(): void {
    if (!launcherUpdateInstalledToastTimer) {
      return;
    }

    clearTimeout(launcherUpdateInstalledToastTimer);
    launcherUpdateInstalledToastTimer = null;
  }

  function showInstalledUpdateToast(version?: string): void {
    launcherUpdateInstalledToastText = version?.trim()
      ? `Обновление лаунчера ${version.trim()} установлено.`
      : "Обновление лаунчера установлено.";
    showLauncherUpdateInstalledToast = true;
    clearLauncherUpdateInstalledToastTimer();
    launcherUpdateInstalledToastTimer = setTimeout(() => {
      showLauncherUpdateInstalledToast = false;
      launcherUpdateInstalledToastTimer = null;
    }, launcherUpdateInstalledToastDurationMs);
  }

  function persistInstalledUpdateToastMarker(version?: string): void {
    if (typeof window === "undefined") {
      return;
    }

    const payload = JSON.stringify({
      version: version?.trim() ?? "",
      at: Date.now(),
    });
    localStorage.setItem(launcherUpdateInstalledToastStorageKey, payload);
  }

  function consumeInstalledUpdateToastMarker(): void {
    if (typeof window === "undefined") {
      return;
    }

    const payload = localStorage.getItem(
      launcherUpdateInstalledToastStorageKey,
    );
    if (!payload) {
      return;
    }

    localStorage.removeItem(launcherUpdateInstalledToastStorageKey);

    try {
      const parsed = JSON.parse(payload) as { version?: string; at?: number };
      const timestamp =
        typeof parsed.at === "number" && Number.isFinite(parsed.at)
          ? parsed.at
          : 0;
      const ageMs = Date.now() - timestamp;
      if (timestamp <= 0 || ageMs > 10 * 60 * 1000) {
        return;
      }

      showInstalledUpdateToast(parsed.version);
    } catch {
      showInstalledUpdateToast();
    }
  }

  async function initializeLauncherAutoUpdateState(): Promise<void> {
    try {
      const settings = await loadLauncherSettingsCommand();
      launcherAutoUpdatesEnabled = settings.autoUpdates ?? true;
    } catch {
      launcherAutoUpdatesEnabled = true;
    }

    scheduleLauncherAutoUpdateChecks();
  }

  function clearLauncherAutoUpdateSchedule(): void {
    if (scheduledAutoUpdateCheckTimer) {
      clearTimeout(scheduledAutoUpdateCheckTimer);
      scheduledAutoUpdateCheckTimer = null;
    }

    if (periodicAutoUpdateCheckTimer) {
      clearInterval(periodicAutoUpdateCheckTimer);
      periodicAutoUpdateCheckTimer = null;
    }
  }

  function scheduleLauncherAutoUpdateChecks(): void {
    clearLauncherAutoUpdateSchedule();

    scheduledAutoUpdateCheckTimer = setTimeout(() => {
      scheduledAutoUpdateCheckTimer = null;
      void checkLauncherUpdates(false);
    }, updateCheckDebounceMs);

    periodicAutoUpdateCheckTimer = setInterval(() => {
      void checkLauncherUpdates(false);
    }, autoUpdatePeriodicCheckIntervalMs);
  }

  async function checkLauncherUpdates(forceCheck: boolean): Promise<void> {
    if (updateCheckInFlight || updateInstallInFlight) {
      return;
    }

    if (!forceCheck && showLauncherUpdatePrompt) {
      return;
    }

    updateCheckInFlight = true;
    launcherUpdateError = "";

    try {
      const update = await checkLauncherUpdateCommand();
      if (!update) {
        if (forceCheck && typeof window !== "undefined") {
          window.alert("Обновлений лаунчера не найдено.");
        }
        return;
      }

      launcherUpdateHandle = update;
      launcherCurrentVersion = update.currentVersion;
      launcherUpdateVersion = update.version;
      launcherUpdateBody = update.body?.trim() ?? "";
      showLauncherUpdatePrompt = true;
      resetLauncherUpdateProgress();
      void installLauncherUpdate();
    } catch (error) {
      console.error("Launcher update check failed:", error);
      const message = formatUpdaterError(error);
      launcherUpdateError = message;
      if (forceCheck && typeof window !== "undefined") {
        window.alert(message);
      }
    } finally {
      updateCheckInFlight = false;
    }
  }

  function dismissLauncherUpdatePrompt(): void {
    showLauncherUpdatePrompt = false;
  }

  async function installLauncherUpdate(): Promise<void> {
    if (!launcherUpdateHandle || updateInstallInFlight) {
      return;
    }

    updateInstallInFlight = true;
    launcherUpdateError = "";
    resetLauncherUpdateProgress();

    try {
      await downloadAndInstallLauncherUpdateCommand(
        launcherUpdateHandle,
        handleLauncherUpdaterProgress,
      );
      persistInstalledUpdateToastMarker(
        launcherUpdateVersion || launcherUpdateHandle.version,
      );
      showLauncherUpdatePrompt = false;
      await restartLauncherCommand();
    } catch (error) {
      console.error("Launcher update install failed:", error);
      launcherUpdateError = formatUpdaterError(error);
    } finally {
      updateInstallInFlight = false;
    }
  }

  function requestManualLauncherUpdateCheck(): void {
    void checkLauncherUpdates(true);
  }

  async function refreshAccountChangeStatus(): Promise<void> {
    const requestId = ++accountChangeStatusRequestId;
    accountChangeStatusLoading = true;
    try {
      const status = await withTimeout(
        getAccountChangeStatus(user.id, user.emailOrLogin),
        accountStatusRequestTimeoutMs,
        "Истекло время ожидания статуса смены аккаунта.",
      );

      if (requestId !== accountChangeStatusRequestId) {
        return;
      }

      accountChangeStatus = status;
    } catch (error) {
      if (requestId !== accountChangeStatusRequestId) {
        return;
      }

      console.error("Failed to load account change status:", error);
      accountChangeStatus = null;
    } finally {
      if (requestId === accountChangeStatusRequestId) {
        accountChangeStatusLoading = false;
      }
    }
  }

  function openAccountSettings(): void {
    void (viewMode === "home" ? playPanelOpenSound() : playSwitchSound());
    viewMode = "account";
    void refreshAccountChangeStatus();
  }

  function openLauncherSettings(): void {
    void (viewMode === "home" ? playPanelOpenSound() : playSwitchSound());
    viewMode = "launcher-settings";
  }

  function openAdminPanel(): void {
    if (!canAccessAdminPanel) {
      return;
    }

    void (viewMode === "home" ? playPanelOpenSound() : playSwitchSound());
    viewMode = "admin";
  }

  function returnHome(): void {
    void playPanelCloseSound();
    viewMode = "home";
  }

  function setActiveTab(nextTab: MainTab, withSound = true): void {
    if (activeTab === nextTab) {
      return;
    }

    activeTab = nextTab;
    if (withSound) {
      void playSwitchSound();
    }
  }

  function updateTheme(nextTheme: ThemeMode): void {
    theme = nextTheme;
    applyTheme(nextTheme);
    localStorage.setItem(themeStorageKey, nextTheme);
  }

  function handleLauncherSettingsSaved(settings: {
    autoUpdates: boolean;
  }): void {
    launcherAutoUpdatesEnabled = settings.autoUpdates;
    scheduleLauncherAutoUpdateChecks();
  }

  async function saveAccountSettings(
    payload: AccountSavePayload,
  ): Promise<string> {
    const requestedNickname = payload.nickname.trim();
    const currentPassword = payload.currentPassword;
    const nextPassword = payload.nextPassword;

    const hasNicknameChange = requestedNickname !== user.nickname.trim();
    const hasPasswordChange = Boolean(currentPassword || nextPassword);

    if (!hasNicknameChange && !hasPasswordChange) {
      throw new Error("Нет изменений для сохранения.");
    }

    if (hasPasswordChange) {
      if (!currentPassword || !nextPassword) {
        throw new Error(
          "Для смены пароля заполните поля текущего и нового пароля.",
        );
      }

      await changePassword(
        {
          currentPassword,
          nextPassword,
        },
        user.id,
        user.emailOrLogin,
      );
    }

    if (hasNicknameChange) {
      if (!requestedNickname) {
        throw new Error("Ник не может быть пустым.");
      }

      await updateAccount(
        {
          nickname: requestedNickname,
        },
        user.id,
        user.emailOrLogin,
      );

      user = {
        ...user,
        nickname: requestedNickname,
      };
      updateStoredSessionNickname(requestedNickname);

      if (discordPresenceReady) {
        void syncDiscordPresence(runningModeName, requestedNickname);
      }
    }

    await refreshAccountChangeStatus();

    if (hasNicknameChange && hasPasswordChange) {
      return "Ник и пароль успешно обновлены.";
    }

    if (hasNicknameChange) {
      return "Ник успешно обновлён.";
    }

    return "Пароль успешно обновлён.";
  }

  function applyTheme(nextTheme: ThemeMode): void {
    document.documentElement.dataset.theme = nextTheme;
  }

  function loadImage(source: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const image = new Image();
      image.onload = () => resolve(image);
      image.onerror = () => reject(new Error("Failed to load skin image."));
      image.src = source;
    });
  }

  async function buildFacePreviewUrl(sourceUrl: string): Promise<string> {
    const source = sourceUrl.trim();
    if (!source) {
      return sourceUrl;
    }

    const image = await loadImage(source);
    const sourceCanvas = document.createElement("canvas");
    sourceCanvas.width = 64;
    sourceCanvas.height = 64;
    const sourceContext = sourceCanvas.getContext("2d");
    if (!sourceContext) {
      return sourceUrl;
    }

    sourceContext.imageSmoothingEnabled = false;
    sourceContext.clearRect(0, 0, 64, 64);
    sourceContext.drawImage(image, 0, 0, 64, 64);

    const faceCanvas = document.createElement("canvas");
    faceCanvas.width = 8;
    faceCanvas.height = 8;
    const faceContext = faceCanvas.getContext("2d");
    if (!faceContext) {
      return sourceUrl;
    }

    faceContext.imageSmoothingEnabled = false;
    faceContext.clearRect(0, 0, 8, 8);
    faceContext.drawImage(sourceCanvas, 8, 8, 8, 8, 0, 0, 8, 8);
    faceContext.drawImage(sourceCanvas, 40, 8, 8, 8, 0, 0, 8, 8);

    const scaledCanvas = document.createElement("canvas");
    scaledCanvas.width = 64;
    scaledCanvas.height = 64;
    const scaledContext = scaledCanvas.getContext("2d");
    if (!scaledContext) {
      return sourceUrl;
    }

    scaledContext.imageSmoothingEnabled = false;
    scaledContext.clearRect(0, 0, 64, 64);
    scaledContext.drawImage(faceCanvas, 0, 0, 8, 8, 0, 0, 64, 64);
    return scaledCanvas.toDataURL("image/png");
  }

  async function setSkinFacePreview(sourceUrl: string): Promise<void> {
    try {
      const faceUrl = await buildFacePreviewUrl(sourceUrl);
      skinPreviewUrl = faceUrl;
      localStorage.setItem(skinPreviewStorageKey, faceUrl);
    } catch {
      skinPreviewUrl = sourceUrl;
      localStorage.setItem(skinPreviewStorageKey, sourceUrl);
    }
  }

  function handleSkinPreviewChange(
    event: CustomEvent<{
      previewUrl: string;
      facePreviewUrl: string;
      uploadedSkinUrl?: string;
    }>,
  ): void {
    const nextPreviewUrl = event.detail.previewUrl?.trim();
    const nextUploadedSkinUrl = event.detail.uploadedSkinUrl?.trim();
    if (nextUploadedSkinUrl) {
      selectedSkinUrl = nextUploadedSkinUrl;
      user = {
        ...user,
        skinUrl: nextUploadedSkinUrl,
      };
    } else if (nextPreviewUrl) {
      selectedSkinUrl = nextPreviewUrl;
    }

    const nextFacePreviewUrl = event.detail.facePreviewUrl?.trim();
    if (nextFacePreviewUrl) {
      skinPreviewUrl = nextFacePreviewUrl;
      localStorage.setItem(skinPreviewStorageKey, nextFacePreviewUrl);
      return;
    }

    if (!nextPreviewUrl) {
      return;
    }

    void setSkinFacePreview(nextPreviewUrl);
  }

  function makeBuildId(name: string): string {
    return name.toLowerCase().replace(/[^a-zа-яё0-9]/gi, "-");
  }

  function formatLaunchError(error: unknown): string {
    if (typeof error === "string" && error.trim()) {
      return error;
    }

    if (error && typeof error === "object") {
      if (
        "message" in error &&
        typeof (error as { message?: unknown }).message === "string"
      ) {
        const message = (error as { message: string }).message.trim();
        if (message) {
          return message;
        }
      }

      try {
        const serialized = JSON.stringify(error);
        if (serialized && serialized !== "{}") {
          return serialized;
        }
      } catch {}
    }

    return "Не удалось запустить игру.";
  }

  function isCancelledOperationError(error: unknown): boolean {
    if (typeof error === "string") {
      return error.includes(downloadCancelledErrorCode);
    }

    if (error && typeof error === "object") {
      const message = (error as { message?: unknown }).message;
      if (
        typeof message === "string" &&
        message.includes(downloadCancelledErrorCode)
      ) {
        return true;
      }
    }

    try {
      return JSON.stringify(error).includes(downloadCancelledErrorCode);
    } catch {
      return false;
    }
  }

  async function syncDiscordPresence(
    activeModeName: string | null,
    nickname: string,
  ): Promise<void> {
    try {
      await updateDiscordPresenceCommand(activeModeName, nickname);
    } catch (error) {
      console.error("Failed to sync Discord RPC state:", error);
    }
  }

  $: if (
    discordPresenceReady &&
    (runningModeName !== lastDiscordPresenceModeName ||
      user.nickname.trim() !== lastDiscordPresenceNickname)
  ) {
    lastDiscordPresenceModeName = runningModeName;
    lastDiscordPresenceNickname = user.nickname.trim();
    void syncDiscordPresence(runningModeName, user.nickname.trim());
  }

  async function refreshRuntimeState(): Promise<void> {
    if (launchInFlight) {
      return;
    }

    try {
      const runtimeState = await getGameRuntimeStateCommand();
      runningModeName = runtimeState.activeModeName;
    } catch (error) {
      console.error("Failed to sync game runtime state:", error);
    }
  }

  async function refreshBuildInstallationStates(): Promise<void> {
    if (launchInFlight) {
      return;
    }

    try {
      const states = await getBuildInstallationStatesCommand(buildNames);
      installedBuildStateByName = Object.fromEntries(
        states.map((state) => [state.modeName, state]),
      );
    } catch (error) {
      console.error("Failed to sync build installation state:", error);
    }
  }

  function clearLaunchProgressTimers(): void {
    if (launchProgressTimer) {
      clearInterval(launchProgressTimer);
      launchProgressTimer = null;
    }

    if (installProgressPollTimer) {
      clearInterval(installProgressPollTimer);
      installProgressPollTimer = null;
    }

    if (launchProgressHideTimer) {
      clearTimeout(launchProgressHideTimer);
      launchProgressHideTimer = null;
    }

    usingRealLaunchProgress = false;
  }

  function stopLaunchProgress(): void {
    clearLaunchProgressTimers();
    showLaunchProgress = false;
    launchProgress = 0;
    launchStatusText = "";
    launchCancelRequested = false;
    launchPendingModeName = null;
  }

  function beginBusyProgress(modeName: string, statusText: string): void {
    clearLaunchProgressTimers();
    launchCancelRequested = false;
    launchPendingModeName = modeName;
    showLaunchProgress = true;
    launchProgress = 6;
    launchStatusText = statusText;

    void syncInstallProgress(modeName);
    installProgressPollTimer = setInterval(() => {
      void syncInstallProgress(modeName);
    }, 140);
  }

  async function syncInstallProgress(modeName: string): Promise<void> {
    try {
      const progressState = await getInstallProgressStateCommand();
      if (!progressState || progressState.modeName !== modeName) {
        return;
      }

      applyInstallProgressState(progressState);
    } catch (error) {
      console.error("Failed to sync install progress state:", error);
    }
  }

  async function cancelActiveDownload(): Promise<void> {
    if (!launchInFlight || !showLaunchProgress || launchCancelRequested) {
      return;
    }

    launchCancelRequested = true;
    launchStatusText = "Отмена загрузки...";

    try {
      await cancelActiveDownloadsCommand();
    } catch (error) {
      launchCancelRequested = false;
      console.error("Failed to cancel active download:", error);
    }
  }

  function applyInstallProgressState(
    progressState: BuildInstallProgressState,
  ): void {
    if (!usingRealLaunchProgress) {
      usingRealLaunchProgress = true;
      if (launchProgressTimer) {
        clearInterval(launchProgressTimer);
        launchProgressTimer = null;
      }
    }

    launchProgress = Math.max(0, Math.min(100, progressState.progressPercent));
    launchStatusText = progressState.stageText || launchStatusText;
  }

  function completeBusyProgress(finalText: string): void {
    if (!showLaunchProgress) {
      launchCancelRequested = false;
      launchPendingModeName = null;
      return;
    }

    clearLaunchProgressTimers();
    launchCancelRequested = false;
    launchProgress = 100;
    launchStatusText = finalText;
    launchProgressHideTimer = setTimeout(() => {
      stopLaunchProgress();
    }, 220);
  }

  async function handleModeLaunch(
    event: CustomEvent<{ modeName: string }>,
  ): Promise<void> {
    await toggleModeRuntime(event.detail.modeName);
  }

  async function handleModeInstall(
    event: CustomEvent<{ modeName: string }>,
  ): Promise<void> {
    await installMode(event.detail.modeName);
  }

  async function launchRecentMode(modeName: string): Promise<void> {
    const build = builds.find((item) => item.name === modeName);
    if (!build) {
      return;
    }

    if (!build.installed || build.updateAvailable) {
      await installMode(modeName);
      return;
    }

    await toggleModeRuntime(modeName);
  }

  function openModeFromRecent(modeName: string): void {
    void playSwitchSound();
    setActiveTab("modes", false);
    openModeRequestIdCounter += 1;
    openModeRequest = {
      id: openModeRequestIdCounter,
      buildId: makeBuildId(modeName),
    };
  }

  function handleOpenModeRequestConsumed(): void {
    openModeRequest = null;
  }

  function buildRecentModeTooltip(mode: BuildItem): string {
    const loader = mode.loader ?? "Forge";
    const version = mode.gameVersion ?? "1.20.1";
    return `${mode.name} • ${loader} ${version}`;
  }

  async function toggleModeRuntime(modeName: string): Promise<void> {
    if (launchInFlight) {
      return;
    }

    const build = builds.find((item) => item.name === modeName);
    const isStoppingCurrentMode = runningModeName === modeName;
    const launchSkinUrl =
      selectedSkinUrl.trim() || user.skinUrl?.trim() || undefined;
    launchInFlight = true;

    if (isStoppingCurrentMode) {
      launchPendingModeName = modeName;
      showLaunchProgress = false;
      launchStatusText = "";
    } else {
      beginBusyProgress(modeName, "Проверка файлов, библиотек и ассетов...");
    }

    try {
      const runtimeState = await toggleGameRuntimeCommand(
        modeName,
        user.id,
        user.nickname,
        build?.gameVersion,
        launchSkinUrl,
      );

      runningModeName = runtimeState.activeModeName;
      void playLaunchSound(runtimeState.running);

      if (runtimeState.running && runtimeState.activeModeName) {
        markModeLaunched(runtimeState.activeModeName);
      }

      if (!isStoppingCurrentMode) {
        completeBusyProgress("Запуск игры...");
      } else {
        launchPendingModeName = null;
      }
    } catch (error) {
      console.error("Game launch failed:", error);
      stopLaunchProgress();
      if (isCancelledOperationError(error)) {
        return;
      }
      const message = formatLaunchError(error);

      if (typeof window !== "undefined") {
        window.alert(message);
      }
    } finally {
      launchCancelRequested = false;
      launchInFlight = false;
    }
  }

  async function installMode(modeName: string): Promise<void> {
    if (launchInFlight) {
      return;
    }

    const build = builds.find((item) => item.name === modeName);
    launchInFlight = true;
    beginBusyProgress(
      modeName,
      build?.updateAvailable
        ? "Обновление и распаковка сборки..."
        : "Загрузка и распаковка сборки...",
    );

    try {
      const installationState = await installBuildCommand(modeName);
      installedBuildStateByName = {
        ...installedBuildStateByName,
        [installationState.modeName]: installationState,
      };
      completeBusyProgress("Сборка установлена");
      void refreshBuildInstallationStates();
    } catch (error) {
      console.error("Build install failed:", error);
      stopLaunchProgress();
      if (isCancelledOperationError(error)) {
        return;
      }
      const message = formatLaunchError(error);

      if (typeof window !== "undefined") {
        window.alert(message);
      }
    } finally {
      launchCancelRequested = false;
      launchInFlight = false;
    }
  }

  function markModeLaunched(modeName: string): void {
    recentModes = [
      modeName,
      ...recentModes.filter((item) => item !== modeName),
    ].slice(0, maxHomeRecentRows);
    localStorage.setItem(recentModesStorageKey, JSON.stringify(recentModes));
  }

  function resolveAssetUrl(fileName: string): string {
    const bundledUrl = bundledAssetUrlsByName[fileName];
    if (bundledUrl) {
      return bundledUrl;
    }

    return `/assets/${fileName}`;
  }

  function resolveCardImageUrl(imageFile: string): string {
    const normalized = imageFile.trim();
    if (
      normalized.toLowerCase().startsWith("https://") ||
      normalized.toLowerCase().startsWith("http://")
    ) {
      return normalized;
    }

    return resolveAssetUrl(normalized);
  }

  function requestPromoCardNavigation(card: HomePromoCard): void {
    const url = card.linkUrl.trim();
    if (!url) {
      return;
    }

    promoNavigationUrl = url;
    showPromoNavigationConfirm = true;
    void playPanelOpenSound();
  }

  function dismissPromoNavigationConfirm(): void {
    showPromoNavigationConfirm = false;
    promoNavigationUrl = "";
    void playPanelCloseSound();
  }

  async function confirmPromoNavigation(): Promise<void> {
    const url = promoNavigationUrl.trim();
    showPromoNavigationConfirm = false;
    promoNavigationUrl = "";
    void playPanelCloseSound();

    if (!url) {
      return;
    }

    try {
      await openExternalUrlCommand(url);
    } catch (error) {
      console.error("Failed to open promo link:", error);
      if (typeof window !== "undefined") {
        window.alert("Не удалось открыть ссылку. Проверь адрес в карточке.");
      }
    }
  }

  function requestSignOut(): void {
    void playPanelOpenSound();
    showSignOutConfirm = true;
  }

  function cancelSignOut(): void {
    void playPanelCloseSound();
    showSignOutConfirm = false;
  }

  function signOut(): void {
    void playPanelCloseSound();
    showSignOutConfirm = false;
    runningModeName = null;
    openModeRequest = null;
    openModeRequestIdCounter = 0;
    dispatch("signOut");
  }
</script>

<main class="launcher-root" bind:this={launcherRootElement}>
  <section class="launcher-shell">
    <aside class="left-nav">
      <div class="nav-buttons">
        <button
          type="button"
          class="nav-button"
          class:active={activeTab === "home"}
          on:click={() => setActiveTab("home")}
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
          on:click={() => setActiveTab("skins")}
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
          on:click={() => setActiveTab("modes")}
          aria-label="Режимы"
          title="Режимы"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <rect x="4" y="5" width="16" height="4" rx="1" />
            <rect x="4" y="10" width="16" height="4" rx="1" />
            <rect x="4" y="15" width="16" height="4" rx="1" />
          </svg>
        </button>

        <div class="nav-divider" aria-hidden="true"></div>

        {#if homeRecentBuilds.length > 0}
          {#each homeRecentBuilds as mode (mode.id)}
            <button
              type="button"
              class="nav-button nav-button-recent"
              on:click={() => openModeFromRecent(mode.name)}
              aria-label={buildRecentModeTooltip(mode)}
              title={buildRecentModeTooltip(mode)}
            >
              <img
                src={resolveAssetUrl(mode.imageFile)}
                alt={`Режим ${mode.name}`}
                class="nav-recent-image"
              />
            </button>
          {/each}
        {/if}
      </div>

      {#if canAccessAdminPanel}
        <button
          type="button"
          class="nav-admin"
          on:click={openAdminPanel}
          aria-label="Админ-панель"
          title="Админ-панель"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 3l7 3v5c0 4.2-2.7 7.9-7 10-4.3-2.1-7-5.8-7-10V6z" />
            <path d="M9.5 12l1.7 1.7L14.8 10" />
          </svg>
        </button>
      {/if}

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

      {#if showLaunchProgress}
        <div class="launch-progress-overlay" role="status" aria-live="polite">
          <div class="launch-progress-panel">
            <div class="launch-progress-title">Подготовка сборки</div>
            <p>{launchStatusText}</p>
            <div class="launch-progress-bar" aria-hidden="true">
              <div
                class="launch-progress-fill"
                style={`width: ${launchProgress}%;`}
              ></div>
            </div>
            <div class="launch-progress-percent">{launchProgress}%</div>
            {#if launchInFlight && launchProgress < 100}
              <div class="launch-progress-actions">
                <button
                  type="button"
                  class="launch-progress-cancel"
                  disabled={launchCancelRequested}
                  on:click={cancelActiveDownload}
                >
                  {launchCancelRequested ? "Отмена..." : "Отменить загрузку"}
                </button>
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <section class="home-layer" class:blurred={viewMode !== "home"}>
        {#key activeTab}
          <div
            class="tab-scene"
            in:fly={{ x: 18, duration: 220, easing: cubicOut }}
            out:fade={{ duration: 120 }}
          >
            {#if activeTab === "home"}
              <section class="tab-placeholder home-panel">
                <section class="home-block recent-block">
                  <h2>Недавно запущенные режимы</h2>
                  {#if homeRecentBuilds.length === 0}
                    <p>Список режимов пока пуст.</p>
                  {:else}
                    <div class="recent-rows">
                      {#each homeRecentBuilds as mode (mode.id)}
                        <div
                          class="recent-row"
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
                          <div class="recent-row-main">
                            <div class="recent-row-icon">
                              <img
                                src={resolveAssetUrl(mode.imageFile)}
                                alt={`Иконка режима ${mode.name}`}
                              />
                            </div>
                            <div class="recent-row-text">
                              <h3>{mode.name}</h3>
                              <p>
                                {mode.loader ?? "Forge"} · {mode.gameVersion ??
                                  "1.20.1"}
                              </p>
                            </div>
                          </div>
                          <button
                            type="button"
                            class="recent-row-launch"
                            class:running={runningModeName === mode.name}
                            disabled={launchInFlight}
                            on:click|stopPropagation={() =>
                              launchRecentMode(mode.name)}
                          >
                            {#if showLaunchProgress && launchPendingModeName === mode.name}
                              {#if mode.updateAvailable}
                                Обновление...
                              {:else if mode.installed}
                                Подготовка...
                              {:else}
                                Установка...
                              {/if}
                            {:else if runningModeName === mode.name}
                              Закрыть
                            {:else if mode.updateAvailable}
                              Обновить
                            {:else if !mode.installed}
                              Установить
                            {:else}
                              Запуск
                            {/if}
                          </button>
                        </div>
                      {/each}
                    </div>
                  {/if}
                </section>

                <section class="home-block promo-block">
                  <div class="promo-grid">
                    {#each homePromoCards as card (card.id)}
                      <a
                        class="promo-card"
                        href={card.linkUrl}
                        target={card.openInNewWindow ? "_blank" : "_self"}
                        rel={card.openInNewWindow
                          ? "noopener noreferrer"
                          : undefined}
                        aria-label={`Открыть: ${card.title}`}
                        on:click|preventDefault={() =>
                          requestPromoCardNavigation(card)}
                      >
                        <div class="promo-image-wrap">
                          <img
                            src={resolveCardImageUrl(card.imageFile)}
                            alt={card.imageAlt}
                            loading="lazy"
                          />
                        </div>
                        <div class="promo-card-body">
                          <h3>{card.title}</h3>
                          <p>{card.description}</p>
                        </div>
                      </a>
                    {/each}
                  </div>
                </section>
              </section>
            {:else if activeTab === "skins"}
              <SkinStudio
                userId={user.id}
                userIdentity={user.emailOrLogin}
                initialStoredSkinUrl={selectedSkinUrl}
                on:skinchange={handleSkinPreviewChange}
              />
            {:else}
              <BuildShowcase
                {builds}
                filterDefinitions={buildFilterDefinitions}
                {assetImageNames}
                {bundledAssetUrlsByName}
                {runningModeName}
                {launchInFlight}
                launchPendingModeName={showLaunchProgress
                  ? launchPendingModeName
                  : null}
                {openModeRequest}
                on:install={handleModeInstall}
                on:launch={handleModeLaunch}
                on:openrequestconsumed={handleOpenModeRequestConsumed}
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
            onSave={saveAccountSettings}
            changeStatus={accountChangeStatus}
            statusLoading={accountChangeStatusLoading}
          />
        {:else if viewMode === "admin"}
          <AdminPanel
            actorUserId={user.id}
            actorIdentity={user.emailOrLogin}
            actorRole={user.role}
            onBack={returnHome}
          />
        {:else}
          <LauncherSettingsPanel
            {theme}
            onBack={returnHome}
            onThemeChange={updateTheme}
            onSaved={handleLauncherSettingsSaved}
            onCheckUpdates={requestManualLauncherUpdateCheck}
            {updateCheckInFlight}
          />
        {/if}
      </section>
    </div>
  {/if}

  {#if showLauncherUpdatePrompt}
    <div
      class="confirm-backdrop"
      role="presentation"
      in:fade={{ duration: 120 }}
      out:fade={{ duration: 100 }}
    >
      <div
        class="confirm-panel update-confirm-panel"
        role="dialog"
        aria-modal="true"
        aria-label="Доступно обновление лаунчера"
        in:fly={{ y: 20, duration: 180, easing: cubicOut }}
        out:fade={{ duration: 120 }}
      >
        <p class="update-confirm-title">Найдено новое обновление лаунчера</p>
        <p>
          Доступна версия <strong>{launcherUpdateVersion}</strong>
          {#if launcherCurrentVersion}
            (сейчас установлена <strong>{launcherCurrentVersion}</strong>)
          {/if}
          .
        </p>
        <p>
          Загрузка началась автоматически. После установки лаунчер
          перезапустится.
        </p>

        {#if launcherUpdateBody}
          <pre class="update-notes">{launcherUpdateBody}</pre>
        {/if}

        {#if updateInstallInFlight}
          <p class="update-progress">
            {#if launcherUpdatePercent !== null}
              Загружаем обновление... {launcherUpdatePercent}% ({formatCompactBytes(
                launcherUpdateDownloadedBytes,
              )}
              {#if launcherUpdateTotalBytes !== null}
                / {formatCompactBytes(launcherUpdateTotalBytes)}
              {/if})
            {:else}
              Загружаем обновление... {formatCompactBytes(
                launcherUpdateDownloadedBytes,
              )}
            {/if}
          </p>
        {/if}

        {#if launcherUpdateError}
          <p class="update-error">{launcherUpdateError}</p>
        {/if}

        <div class="confirm-actions">
          <button
            type="button"
            class="confirm-ack"
            on:click={dismissLauncherUpdatePrompt}
          >
            Ок
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if showLauncherUpdateInstalledToast}
    <div
      class="update-installed-toast"
      role="status"
      aria-live="polite"
      in:fly={{ y: -18, duration: 260, easing: cubicOut }}
      out:fade={{ duration: 220 }}
    >
      {launcherUpdateInstalledToastText}
    </div>
  {/if}

  {#if showPromoNavigationConfirm}
    <div
      class="confirm-backdrop"
      role="presentation"
      in:fade={{ duration: 120 }}
      out:fade={{ duration: 100 }}
    >
      <div
        class="confirm-panel promo-navigation-panel"
        role="dialog"
        aria-modal="true"
        aria-label="Подтверждение перехода по внешней ссылке"
        in:fly={{ y: 20, duration: 180, easing: cubicOut }}
        out:fade={{ duration: 120 }}
      >
        <button
          type="button"
          class="promo-navigation-close"
          aria-label="Закрыть"
          on:click={dismissPromoNavigationConfirm}
        >
          ×
        </button>
        <h3 class="promo-navigation-title">Вы покидаете TBW Launcher</h3>
        <p class="promo-navigation-subtitle">
          Эта ссылка приведёт вас на следующий сайт
        </p>
        <div class="promo-navigation-url-box">{promoNavigationUrl}</div>
        <div class="promo-navigation-actions">
          <button
            type="button"
            class="promo-navigation-back"
            on:click={dismissPromoNavigationConfirm}>Вернуться</button
          >
          <button
            type="button"
            class="promo-navigation-visit"
            on:click={confirmPromoNavigation}>Посетить сайт</button
          >
        </div>
      </div>
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

  .nav-divider {
    width: 76%;
    justify-self: center;
    height: 1px;
    margin: 4px 0 2px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--line) 78%, var(--accent) 22%);
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

  .nav-button-recent {
    overflow: hidden;
    padding: 0;
  }

  .nav-recent-image {
    width: 100%;
    height: 100%;
    display: block;
    object-fit: cover;
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

  .nav-admin {
    position: absolute;
    left: 50%;
    bottom: 66px;
    transform: translateX(-50%);
    width: 48px;
    height: 48px;
    border-radius: 12px;
    border: 1px solid rgba(105, 199, 154, 0.55);
    background: rgba(71, 172, 136, 0.12);
    color: #8fe8c3;
    display: inline-grid;
    place-items: center;
    cursor: pointer;
    transition:
      transform 0.16s ease,
      border-color 0.16s ease,
      box-shadow 0.16s ease,
      background-color 0.16s ease;
  }

  .nav-admin:hover {
    transform: translateX(-50%) scale(1.045);
    border-color: rgba(132, 232, 186, 0.92);
    box-shadow: 0 0 0 1px rgba(112, 216, 171, 0.34);
    background: rgba(71, 172, 136, 0.21);
  }

  .nav-admin:active {
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

  .nav-admin svg {
    width: 23px;
    height: 23px;
    stroke: currentColor;
    fill: none;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .launcher-card {
    position: relative;
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

  .launch-progress-overlay {
    position: absolute;
    inset: 0;
    z-index: 4;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(8, 14, 22, 0.58);
    backdrop-filter: blur(6px);
  }

  .launch-progress-panel {
    width: min(420px, 100%);
    border-radius: 16px;
    border: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface-main) 92%, #0b1422 8%);
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.32);
    padding: 18px 18px 16px;
  }

  .launch-progress-title {
    font-size: 0.98rem;
    font-weight: 800;
    letter-spacing: 0.01em;
  }

  .launch-progress-panel p {
    margin: 8px 0 14px;
    font-size: 0.84rem;
    color: var(--text-muted);
  }

  .launch-progress-bar {
    width: 100%;
    height: 10px;
    border-radius: 999px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .launch-progress-fill {
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, #69d9b7 0%, #7ad5f4 52%, #a4b8ff 100%);
    box-shadow: 0 0 18px rgba(122, 213, 244, 0.28);
    transition: width 0.18s ease;
  }

  .launch-progress-percent {
    margin-top: 10px;
    text-align: right;
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--text-muted);
  }

  .launch-progress-actions {
    margin-top: 12px;
    display: flex;
    justify-content: flex-end;
  }

  .launch-progress-cancel {
    border: 1px solid rgba(255, 130, 130, 0.55);
    background: rgba(255, 90, 90, 0.15);
    color: #ffd7d7;
    border-radius: 10px;
    padding: 8px 12px;
    font: inherit;
    font-size: 0.8rem;
    font-weight: 700;
    cursor: pointer;
    transition:
      background 0.14s ease,
      border-color 0.14s ease,
      transform 0.14s ease;
  }

  .launch-progress-cancel:hover:not(:disabled) {
    background: rgba(255, 90, 90, 0.22);
    border-color: rgba(255, 140, 140, 0.75);
    transform: translateY(-1px);
  }

  .launch-progress-cancel:disabled {
    opacity: 0.72;
    cursor: default;
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

  .home-panel {
    min-height: 0;
    height: 100%;
    overflow: auto;
    display: grid;
    align-content: start;
    gap: 18px;
    padding-top: 10px;
    padding-bottom: 10px;
  }

  .home-panel::-webkit-scrollbar {
    width: 8px;
  }

  .home-panel::-webkit-scrollbar-track {
    background: color-mix(in srgb, var(--surface-alt) 84%, transparent);
    border-radius: 999px;
  }

  .home-panel::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--line) 80%, var(--accent) 20%);
    border-radius: 999px;
  }

  .home-panel::-webkit-scrollbar-thumb:hover {
    background: color-mix(in srgb, var(--accent) 46%, var(--line) 54%);
  }

  .tab-placeholder h2 {
    margin: 0 0 0;
  }

  .home-block {
    display: grid;
    gap: 10px;
  }

  .tab-placeholder p {
    margin: 0;
    color: var(--text-muted);
  }

  .home-block h2 {
    margin-top: -2px;
  }

  .recent-rows {
    display: grid;
    gap: 10px;
    margin-top: 2px;
  }

  .recent-row {
    border: 1px solid var(--line);
    background: var(--surface-main);
    border-radius: 10px;
    padding: 10px 12px;
    color: var(--text-main);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
    cursor: pointer;
    transition:
      transform 0.14s ease,
      border-color 0.16s ease,
      box-shadow 0.16s ease,
      background-color 0.16s ease;
  }

  .recent-row:hover {
    transform: translateY(-1px);
    border-color: var(--accent);
    background: var(--surface-elevated);
    box-shadow: 0 8px 18px rgba(9, 16, 26, 0.2);
  }

  .recent-row:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 72%, transparent);
    outline-offset: 2px;
  }

  .recent-row:active {
    transform: translateY(0);
  }

  .recent-row-main {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .recent-row-icon {
    width: 44px;
    height: 44px;
    flex: 0 0 auto;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid var(--line);
    background: var(--surface-alt);
  }

  .recent-row-icon img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .recent-row-text {
    min-width: 0;
    display: grid;
    gap: 2px;
  }

  .recent-row-text h3 {
    margin: 0;
    font-size: 0.92rem;
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .recent-row-text p {
    margin: 0;
    font-size: 0.76rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .recent-row-launch {
    flex: 0 0 auto;
    min-width: 112px;
    display: inline-flex;
    justify-content: center;
    align-items: center;
    border: none;
    border-radius: 7px;
    padding: 5px 8px;
    font: inherit;
    font-size: 0.75rem;
    font-weight: 700;
    color: #0b1422;
    background: linear-gradient(135deg, #9ef7ce, #84bffb);
    cursor: pointer;
    transition:
      transform 0.14s ease,
      box-shadow 0.14s ease,
      filter 0.14s ease;
  }

  .recent-row-launch:hover {
    transform: translateY(-1px);
    box-shadow: 0 3px 10px rgba(72, 191, 180, 0.28);
    filter: brightness(1.05);
  }

  .recent-row-launch.running {
    color: #ffeaea;
    background: linear-gradient(135deg, #eb5d5d, #b92020);
    box-shadow: 0 4px 12px rgba(168, 38, 38, 0.28);
  }

  .recent-row-launch.running:hover {
    box-shadow: 0 4px 12px rgba(168, 38, 38, 0.38);
  }

  .recent-row-launch:disabled {
    cursor: wait;
    transform: none;
    filter: none;
    box-shadow: none;
    opacity: 0.86;
  }

  .promo-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .promo-card {
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--surface-main);
    text-decoration: none;
    color: var(--text-main);
    overflow: hidden;
    display: grid;
    grid-template-rows: auto 1fr;
    transition:
      transform 0.16s ease,
      border-color 0.16s ease,
      box-shadow 0.16s ease,
      background-color 0.16s ease;
  }

  .promo-card:hover {
    transform: translateY(-2px);
    border-color: var(--accent);
    box-shadow: 0 14px 24px rgba(9, 16, 26, 0.26);
    background: var(--surface-elevated);
  }

  .promo-card:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 72%, transparent);
    outline-offset: 2px;
  }

  .promo-image-wrap {
    width: 100%;
    aspect-ratio: 16 / 9;
    border-bottom: 1px solid var(--line);
    background: var(--surface-alt);
    overflow: hidden;
  }

  .promo-image-wrap img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .promo-card-body {
    padding: 10px 11px 12px;
    display: grid;
    gap: 6px;
  }

  .promo-card-body h3 {
    margin: 0;
    font-size: 0.92rem;
    line-height: 1.25;
  }

  .promo-card-body p {
    margin: 0;
    font-size: 0.8rem;
    line-height: 1.35;
    color: var(--text-muted);
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

  .update-confirm-panel {
    width: min(560px, 94vw);
  }

  .promo-navigation-panel {
    width: min(560px, 94vw);
    position: relative;
    border-color: color-mix(in srgb, var(--line-strong) 82%, var(--accent) 18%);
    background: color-mix(in srgb, var(--surface-main) 86%, var(--background-main) 14%);
    padding: 18px;
    gap: 12px;
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

  .update-confirm-title {
    font-size: 1.05rem;
    font-weight: 700;
  }

  .promo-navigation-title {
    margin: 0;
    color: var(--text-main);
    font-size: 1.85rem;
    font-weight: 800;
    line-height: 1.2;
    padding-right: 32px;
  }

  .promo-navigation-subtitle {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.95rem;
    line-height: 1.35;
  }

  .promo-navigation-url-box {
    border: 1px solid var(--line);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-alt) 82%, var(--background-main) 18%);
    color: var(--text-main);
    padding: 12px 14px;
    font-family: "Consolas", "Menlo", "Monaco", monospace;
    font-size: 0.92rem;
    line-height: 1.35;
    word-break: break-word;
  }

  .promo-navigation-close {
    position: absolute;
    top: 10px;
    right: 10px;
    width: 30px;
    height: 30px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--text-muted);
    font-size: 1.35rem;
    line-height: 1;
    cursor: pointer;
    display: grid;
    place-items: center;
  }

  .promo-navigation-close:hover {
    background: color-mix(in srgb, var(--line) 42%, transparent);
    color: var(--text-main);
  }

  .promo-navigation-actions {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    margin-top: 4px;
  }

  .promo-navigation-back,
  .promo-navigation-visit {
    border: none;
    border-radius: 10px;
    min-height: 42px;
    font: inherit;
    font-size: 0.98rem;
    font-weight: 700;
    cursor: pointer;
    transition:
      filter 0.14s ease,
      transform 0.14s ease,
      box-shadow 0.14s ease;
  }

  .promo-navigation-back {
    background: color-mix(in srgb, var(--surface-alt) 74%, var(--background-main) 26%);
    color: var(--text-main);
    border: 1px solid var(--line-strong);
  }

  .promo-navigation-visit {
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--accent) 66%, #6ea8ff 34%),
      color-mix(in srgb, var(--accent-soft) 70%, #4f67f2 30%)
    );
    color: #f6f8ff;
    border: 1px solid color-mix(in srgb, var(--accent) 72%, var(--line-strong) 28%);
  }

  .promo-navigation-back:hover,
  .promo-navigation-visit:hover {
    filter: brightness(1.06);
    transform: translateY(-1px);
  }

  .update-notes {
    margin: 0;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-alt) 86%, #081120 14%);
    color: var(--text-soft);
    font: inherit;
    font-size: 0.88rem;
    line-height: 1.45;
    padding: 10px;
    max-height: 150px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .update-progress {
    color: var(--text-soft);
    font-size: 0.92rem;
  }

  .update-error {
    color: #ff8f8f;
    font-size: 0.9rem;
  }

  .update-installed-toast {
    position: fixed;
    top: 14px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 7;
    max-width: min(560px, calc(100vw - 24px));
    border-radius: 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 58%, #ffffff 12%);
    background: color-mix(in srgb, var(--surface-main) 80%, #0f2b1f 20%);
    color: #ddffe8;
    font-size: 0.9rem;
    font-weight: 600;
    line-height: 1.35;
    padding: 10px 14px;
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--accent) 30%, transparent),
      0 10px 24px rgba(0, 0, 0, 0.34);
    animation: update-toast-breathe 1.6s ease-in-out infinite;
  }

  @keyframes update-toast-breathe {
    0%,
    100% {
      box-shadow:
        0 0 0 1px color-mix(in srgb, var(--accent) 30%, transparent),
        0 10px 24px rgba(0, 0, 0, 0.34);
    }
    50% {
      box-shadow:
        0 0 0 1px color-mix(in srgb, var(--accent) 55%, transparent),
        0 12px 28px rgba(0, 0, 0, 0.4);
    }
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  .confirm-cancel,
  .confirm-submit,
  .confirm-ack {
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
    border: 1px solid rgba(48, 198, 116, 0.55);
    background: rgba(56, 255, 116, 0.16);
    color: #48d77a;
  }

  .confirm-ack {
    border: 1px solid rgba(89, 201, 132, 0.72);
    background: linear-gradient(135deg, #9ef7ce, #59c984);
    color: #062413;
    font-weight: 700;
  }

  .confirm-cancel:hover {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 36%, transparent);
    transform: translateY(-1px);
  }

  .confirm-submit:hover {
    border-color: rgba(29, 255, 100, 0.92);
    box-shadow: 0 0 0 1px rgba(61, 255, 181, 0.32);
    background: rgba(58, 255, 110, 0.24);
    transform: translateY(-1px);
  }

  .confirm-ack:hover {
    border-color: rgba(133, 240, 174, 0.92);
    box-shadow: 0 0 0 1px rgba(89, 201, 132, 0.35);
    filter: brightness(1.03);
    transform: translateY(-1px);
  }

  .confirm-cancel:disabled,
  .confirm-submit:disabled,
  .confirm-ack:disabled {
    opacity: 0.65;
    cursor: default;
    transform: none;
    box-shadow: none;
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

    .home-panel {
      gap: 14px;
    }

    .recent-row {
      padding: 9px 10px;
    }

    .recent-row-icon {
      width: 40px;
      height: 40px;
    }

    .recent-row-launch {
      min-width: 96px;
      padding: 5px 7px;
      font-size: 0.72rem;
    }

    .promo-grid {
      grid-template-columns: 1fr;
    }

    .promo-navigation-title {
      font-size: 1.55rem;
    }

    .promo-navigation-actions {
      grid-template-columns: 1fr;
    }

    .update-installed-toast {
      top: 10px;
      border-radius: 10px;
      padding: 9px 12px;
      font-size: 0.85rem;
    }
  }
</style>
