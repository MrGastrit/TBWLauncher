<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import SkinCharacterPreview from "./SkinCharacterPreview.svelte";
  import {
    setSkinUrl,
    uploadSkin,
    uploadSkinData,
  } from "../../auth/services/auth-service";
  import defaultSkinSteveUrl from "../../../assets/skins/default/steve.png";
  import defaultSkinAlexUrl from "../../../assets/skins/default/alex.png";

  type ArmType = "wide" | "slim";

  type SavedSkin = {
    id: string;
    name: string;
    skinUrl: string;
    armType: ArmType;
    storedSkinUrl?: string;
  };

  type SkinStudioEvents = {
    skinchange: {
      previewUrl: string;
      facePreviewUrl: string;
      uploadedSkinUrl?: string;
    };
  };

  const DEFAULT_SKINS: Array<{ id: string; name: string; url: string }> = [
    {
      id: "default-steve",
      name: "Steve",
      url: defaultSkinSteveUrl,
    },
    {
      id: "default-alex",
      name: "Alex",
      url: defaultSkinAlexUrl,
    },
  ];

  const dispatch = createEventDispatcher<SkinStudioEvents>();
  export let userId = "";
  export let userIdentity = "";
  export let initialStoredSkinUrl = "";

  let fileInputElement: HTMLInputElement;
  let savedSkins: SavedSkin[] = [];
  let selectedSkinId = "";
  let selectionDispatchToken = 0;
  let lastSyncedSkinUrl = "";
  let skipNextSkinSync = true;
  let contextMenuSkinId: string | null = null;
  let contextMenuX = 0;
  let contextMenuY = 0;
  let pendingDeleteSkinId: string | null = null;
  let showDeleteConfirm = false;

  onMount(() => {
    void initializeDefaultSkins();

    const handlePointerDown = (event: PointerEvent): void => {
      const target = event.target as HTMLElement | null;
      if (target?.closest(".skin-context-menu")) {
        return;
      }

      contextMenuSkinId = null;
    };
    const handleEscape = (event: KeyboardEvent): void => {
      if (event.key !== "Escape") {
        return;
      }

      contextMenuSkinId = null;
      if (showDeleteConfirm) {
        showDeleteConfirm = false;
        pendingDeleteSkinId = null;
      }
    };
    const handleWindowResize = (): void => {
      contextMenuSkinId = null;
    };

    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleEscape);
    window.addEventListener("resize", handleWindowResize);

    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleEscape);
      window.removeEventListener("resize", handleWindowResize);
    };
  });

  function getUserScopeKey(): string {
    const normalizedUserId = (userId ?? "").trim().toLowerCase();
    if (normalizedUserId) {
      return normalizedUserId;
    }

    const normalizedIdentity = (userIdentity ?? "").trim().toLowerCase();
    if (normalizedIdentity) {
      return normalizedIdentity.replace(/[^a-z0-9._@-]/g, "_");
    }

    return "";
  }

  function getSavedSkinsStorageKey(): string {
    const userScopeKey = getUserScopeKey();
    return userScopeKey
      ? `tbwlauncher-skins-${userScopeKey}`
      : "tbwlauncher-skins-anonymous";
  }

  function getSelectedSkinStorageKey(): string {
    return `${getSavedSkinsStorageKey()}-selected`;
  }

  function loadStoredSkins(): SavedSkin[] {
    if (typeof window === "undefined") {
      return [];
    }

    const raw = localStorage.getItem(getSavedSkinsStorageKey());
    if (!raw) {
      return [];
    }

    try {
      const parsed = JSON.parse(raw) as SavedSkin[];
      if (!Array.isArray(parsed)) {
        return [];
      }

      return parsed
        .filter(
          (skin) =>
            typeof skin.id === "string" &&
            typeof skin.name === "string" &&
            typeof skin.skinUrl === "string" &&
            (skin.armType === "wide" || skin.armType === "slim"),
        )
        .map((skin) => ({
          ...skin,
          storedSkinUrl:
            typeof skin.storedSkinUrl === "string"
              ? skin.storedSkinUrl
              : undefined,
        }));
    } catch {
      return [];
    }
  }

  function persistSkinsToStorage(nextSkins: SavedSkin[]): void {
    if (typeof window === "undefined") {
      return;
    }

    const uploadOnlySkins = nextSkins.filter(
      (skin) => !skin.id.startsWith("default-"),
    );
    localStorage.setItem(
      getSavedSkinsStorageKey(),
      JSON.stringify(uploadOnlySkins),
    );
  }

  function loadSelectedSkinId(): string {
    if (typeof window === "undefined") {
      return "";
    }

    return localStorage.getItem(getSelectedSkinStorageKey()) ?? "";
  }

  function persistSelectedSkinId(skinId: string): void {
    if (typeof window === "undefined") {
      return;
    }

    const normalizedSkinId = skinId.trim();
    if (!normalizedSkinId) {
      localStorage.removeItem(getSelectedSkinStorageKey());
      return;
    }

    localStorage.setItem(getSelectedSkinStorageKey(), normalizedSkinId);
  }

  async function initializeDefaultSkins(): Promise<void> {
    try {
      const defaults: SavedSkin[] = await Promise.all(
        DEFAULT_SKINS.map(async (entry) => {
          const normalized = await normalizeSkin(entry.url);
          return {
            id: entry.id,
            name: entry.name,
            skinUrl: normalized.skinUrl,
            armType: normalized.armType,
            storedSkinUrl: undefined,
          };
        }),
      );

      const storedSkins = loadStoredSkins();
      const mergedSkins: SavedSkin[] = [...defaults, ...storedSkins];
      const normalizedInitialStoredSkinUrl = initialStoredSkinUrl.trim();
      const storedSelectedSkinId = loadSelectedSkinId();
      const preferredSkin = normalizedInitialStoredSkinUrl
        ? mergedSkins.find(
            (skin) =>
              skin.storedSkinUrl?.trim() === normalizedInitialStoredSkinUrl,
          )
        : null;
      const storedSelectedSkin = storedSelectedSkinId
        ? mergedSkins.find((skin) => skin.id === storedSelectedSkinId)
        : null;

      savedSkins = mergedSkins;
      if (!selectedSkinId && mergedSkins.length > 0) {
        selectedSkinId =
          storedSelectedSkin?.id ??
          preferredSkin?.id ??
          defaults[0]?.id ??
          mergedSkins[0].id;
      }
      if (normalizedInitialStoredSkinUrl) {
        lastSyncedSkinUrl = normalizedInitialStoredSkinUrl;
      }
    } catch (error) {
      console.error("Failed to prepare starter skins:", error);
      savedSkins = [];
      selectedSkinId = "";
    }
  }

  function openFilePicker(): void {
    fileInputElement?.click();
  }

  async function handleFileChange(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) {
      return;
    }

    try {
      const skin = await createSkinFromFile(file);
      const facePreviewUrl = await createFacePreviewDataUrl(skin.skinUrl);
      const nativeFilePath = getNativeFilePath(file);
      const normalizedUserId = (userId ?? "").trim();
      const normalizedIdentity = (userIdentity ?? "").trim();
      let nextSkin = skin;

      if (nativeFilePath && (normalizedUserId || normalizedIdentity)) {
        try {
          const uploadedSkinUrl = await uploadSkin(
            normalizedUserId,
            nativeFilePath,
            normalizedIdentity || undefined,
          );
          lastSyncedSkinUrl = uploadedSkinUrl;
          nextSkin = {
            ...skin,
            storedSkinUrl: uploadedSkinUrl,
          };
          dispatch("skinchange", {
            previewUrl: nextSkin.skinUrl,
            facePreviewUrl,
            uploadedSkinUrl,
          });
        } catch (uploadError) {
          console.error("Failed to upload the selected skin:", uploadError);
        }
      } else if (normalizedUserId || normalizedIdentity) {
        try {
          const uploadedSkinUrl = await uploadSkinData(
            normalizedUserId,
            file.name,
            skin.skinUrl,
            normalizedIdentity || undefined,
          );
          lastSyncedSkinUrl = uploadedSkinUrl;
          nextSkin = {
            ...skin,
            storedSkinUrl: uploadedSkinUrl,
          };
          dispatch("skinchange", {
            previewUrl: nextSkin.skinUrl,
            facePreviewUrl,
            uploadedSkinUrl,
          });
        } catch (uploadError) {
          console.error("Failed to upload skin data URL:", uploadError);
        }
      }

      const nextSkins = [...savedSkins, nextSkin];
      savedSkins = nextSkins;
      persistSkinsToStorage(nextSkins);
      selectedSkinId = nextSkin.id;
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Не удалось загрузить скин.";
      alert(message);
    } finally {
      input.value = "";
    }
  }

  async function createSkinFromFile(file: File): Promise<SavedSkin> {
    const rawUrl = await readFileAsDataUrl(file);
    const normalized = await normalizeSkin(rawUrl);
    const fileName = file.name.replace(/\.[^.]+$/, "").trim() || "Новый скин";

    return {
      id:
        typeof crypto !== "undefined" && "randomUUID" in crypto
          ? crypto.randomUUID()
          : `${Date.now()}-${Math.random().toString(16).slice(2)}`,
      name: fileName,
      armType: normalized.armType,
      skinUrl: normalized.skinUrl,
    };
  }

  function readFileAsDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => {
        if (typeof reader.result === "string") {
          resolve(reader.result);
          return;
        }

        reject(new Error("Не удалось прочитать файл скина."));
      };
      reader.onerror = () =>
        reject(new Error("Не удалось прочитать файл скина."));
      reader.readAsDataURL(file);
    });
  }

  function loadImage(source: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const image = new Image();
      image.onload = () => resolve(image);
      image.onerror = () =>
        reject(new Error("Файл не похож на валидный PNG-скин."));
      image.src = source;
    });
  }

  async function createFacePreviewDataUrl(skinSource: string): Promise<string> {
    const image = await loadImage(skinSource);
    const sourceCanvas = document.createElement("canvas");
    sourceCanvas.width = 64;
    sourceCanvas.height = 64;
    const sourceContext = sourceCanvas.getContext("2d");
    if (!sourceContext) {
      throw new Error(
        "Не удалось подготовить источник для предпросмотра лица.",
      );
    }

    sourceContext.imageSmoothingEnabled = false;
    sourceContext.clearRect(0, 0, 64, 64);
    sourceContext.drawImage(image, 0, 0, 64, 64);

    const faceCanvas = document.createElement("canvas");
    faceCanvas.width = 8;
    faceCanvas.height = 8;
    const faceContext = faceCanvas.getContext("2d");
    if (!faceContext) {
      throw new Error("Не удалось собрать лицо для аватарки.");
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
      throw new Error("Не удалось увеличить лицо для аватарки.");
    }

    scaledContext.imageSmoothingEnabled = false;
    scaledContext.clearRect(0, 0, 64, 64);
    scaledContext.drawImage(faceCanvas, 0, 0, 8, 8, 0, 0, 64, 64);
    return scaledCanvas.toDataURL("image/png");
  }

  function copyArea(
    context: CanvasRenderingContext2D,
    sourceX: number,
    sourceY: number,
    width: number,
    height: number,
    targetX: number,
    targetY: number,
  ): void {
    const snapshot = context.getImageData(sourceX, sourceY, width, height);
    context.putImageData(snapshot, targetX, targetY);
  }

  function hasTransparentPixels(
    context: CanvasRenderingContext2D,
    startX: number,
    startY: number,
    width: number,
    height: number,
  ): boolean {
    const imageData = context.getImageData(startX, startY, width, height).data;
    for (let index = 3; index < imageData.length; index += 4) {
      if (imageData[index] < 16) {
        return true;
      }
    }

    return false;
  }

  async function normalizeSkin(
    source: string,
  ): Promise<{ skinUrl: string; armType: ArmType }> {
    const image = await loadImage(source);
    const canvas = document.createElement("canvas");
    canvas.width = 64;
    canvas.height = 64;
    const context = canvas.getContext("2d");

    if (!context) {
      throw new Error("Не удалось подготовить холст для скина.");
    }

    context.imageSmoothingEnabled = false;
    context.clearRect(0, 0, 64, 64);

    if (image.naturalWidth === 64 && image.naturalHeight === 32) {
      context.drawImage(image, 0, 0, 64, 32);
      copyArea(context, 0, 16, 16, 16, 16, 48);
      copyArea(context, 40, 16, 16, 16, 32, 48);
    } else if (image.naturalWidth === 64 && image.naturalHeight === 64) {
      context.drawImage(image, 0, 0, 64, 64);
    } else {
      throw new Error("Поддерживаются только скины 64x64 или 64x32.");
    }

    const armType: ArmType =
      hasTransparentPixels(context, 54, 20, 2, 12) ||
      hasTransparentPixels(context, 46, 52, 2, 12)
        ? "slim"
        : "wide";

    return {
      skinUrl: canvas.toDataURL("image/png"),
      armType,
    };
  }

  function selectSkin(skinId: string): void {
    contextMenuSkinId = null;
    selectedSkinId = skinId;
    persistSelectedSkinId(skinId);
  }

  function isDefaultSkin(skin: SavedSkin): boolean {
    return skin.id.startsWith("default-");
  }

  function openSkinContextMenu(event: MouseEvent, skin: SavedSkin): void {
    if (isDefaultSkin(skin)) {
      contextMenuSkinId = null;
      return;
    }

    event.preventDefault();

    const menuWidth = 160;
    const menuHeight = 46;
    const safeX = Math.max(
      8,
      Math.min(event.clientX, window.innerWidth - menuWidth - 8),
    );
    const safeY = Math.max(
      8,
      Math.min(event.clientY, window.innerHeight - menuHeight - 8),
    );

    contextMenuSkinId = skin.id;
    contextMenuX = safeX;
    contextMenuY = safeY;
  }

  function openDeleteConfirmFromContextMenu(): void {
    if (!contextMenuSkinId) {
      return;
    }

    pendingDeleteSkinId = contextMenuSkinId;
    showDeleteConfirm = true;
    contextMenuSkinId = null;
  }

  function cancelDeleteSkin(): void {
    showDeleteConfirm = false;
    pendingDeleteSkinId = null;
  }

  function confirmDeleteSkin(): void {
    if (!pendingDeleteSkinId) {
      return;
    }

    const deletedSkin = savedSkins.find((skin) => skin.id === pendingDeleteSkinId);
    savedSkins = savedSkins.filter((skin) => skin.id !== pendingDeleteSkinId);

    if (selectedSkinId === pendingDeleteSkinId) {
      selectedSkinId = savedSkins[0]?.id ?? "";
    }

    if (
      deletedSkin?.storedSkinUrl &&
      deletedSkin.storedSkinUrl.trim() === lastSyncedSkinUrl
    ) {
      lastSyncedSkinUrl = "";
    }

    persistSkinsToStorage(savedSkins);
    showDeleteConfirm = false;
    pendingDeleteSkinId = null;
  }

  function getNativeFilePath(file: File): string | null {
    const withPath = file as File & { path?: string };
    if (typeof withPath.path === "string" && withPath.path.trim()) {
      return withPath.path;
    }

    return null;
  }

  $: selectedSkin =
    savedSkins.find((skin) => skin.id === selectedSkinId) ??
    savedSkins[0] ??
    null;

  async function emitSelectedSkinChange(skin: SavedSkin): Promise<void> {
    const currentToken = ++selectionDispatchToken;
    try {
      const facePreviewUrl = await createFacePreviewDataUrl(skin.skinUrl);
      if (
        currentToken !== selectionDispatchToken ||
        selectedSkin?.id !== skin.id
      ) {
        return;
      }

      dispatch("skinchange", {
        previewUrl: skin.skinUrl,
        facePreviewUrl,
        uploadedSkinUrl: skin.storedSkinUrl,
      });
    } catch (error) {
      console.error("Failed to prepare face preview:", error);
      if (
        currentToken !== selectionDispatchToken ||
        selectedSkin?.id !== skin.id
      ) {
        return;
      }

      dispatch("skinchange", {
        previewUrl: skin.skinUrl,
        facePreviewUrl: skin.skinUrl,
        uploadedSkinUrl: skin.storedSkinUrl,
      });
    }
  }

  async function syncSelectedSkinUrl(skin: SavedSkin): Promise<void> {
    const normalizedUserId = (userId ?? "").trim();
    const normalizedIdentity = (userIdentity ?? "").trim();
    if (!normalizedUserId && !normalizedIdentity) {
      return;
    }

    let normalizedSkinUrl = skin.storedSkinUrl?.trim() ?? "";
    if (!normalizedSkinUrl) {
      try {
        normalizedSkinUrl = await uploadSkinData(
          normalizedUserId,
          skin.name,
          skin.skinUrl,
          normalizedIdentity || undefined,
        );
        savedSkins = savedSkins.map((item) =>
          item.id === skin.id
            ? { ...item, storedSkinUrl: normalizedSkinUrl }
            : item,
        );
        persistSkinsToStorage(savedSkins);
      } catch (error) {
        console.error("Failed to upload selected skin data:", error);
        return;
      }
    }

    if (!normalizedSkinUrl || normalizedSkinUrl === lastSyncedSkinUrl) {
      return;
    }

    try {
      await setSkinUrl(
        normalizedUserId,
        normalizedSkinUrl,
        normalizedIdentity || undefined,
      );
      lastSyncedSkinUrl = normalizedSkinUrl;
    } catch (error) {
      console.error("Failed to sync selected skin URL:", error);
    }
  }

  $: if (selectedSkin) {
    persistSkinsToStorage(savedSkins);
    persistSelectedSkinId(selectedSkin.id);
    if (skipNextSkinSync) {
      skipNextSkinSync = false;
    } else {
      void syncSelectedSkinUrl(selectedSkin);
    }
    void emitSelectedSkinChange(selectedSkin);
  }
</script>

<section class="skin-studio">
  <section class="skin-library">
    <div class="skin-library-head">
      <h2>Skins</h2>
      <p>Меню выбора скинов</p>
    </div>

    <input
      bind:this={fileInputElement}
      class="skin-file-input"
      type="file"
      accept=".png,image/png"
      on:change={handleFileChange}
    />

    <div class="skin-grid">
      <button
        type="button"
        class="skin-card add-skin-card"
        on:click={openFilePicker}
      >
        <span class="add-skin-plus">+</span>
        <span>Добавить скин</span>
      </button>

      {#each savedSkins as skin (skin.id)}
        <button
          type="button"
          class="skin-card saved-skin-card"
          class:selected={selectedSkin?.id === skin.id}
          on:click={() => selectSkin(skin.id)}
          on:contextmenu={(event) => openSkinContextMenu(event, skin)}
          aria-label={`Выбрать скин ${skin.name}`}
        >
          <div class="skin-card-preview">
            <SkinCharacterPreview
              skinUrl={skin.skinUrl}
              armType={skin.armType}
              interactive={false}
              scale={0.82}
              initialYaw={150}
            />
          </div>
          <div class="skin-card-footer">
            <span>{skin.name}</span>
            <small>{skin.armType}</small>
          </div>
        </button>
      {/each}
    </div>
  </section>

  <aside class="skin-preview-panel">
    <div class="skin-preview-head">
      <h3>Предпросмотр</h3>
      {#if selectedSkin}
        <span class="skin-arm-chip">
          {selectedSkin.armType === "slim" ? "Slim arms" : "Wide arms"}
        </span>
      {/if}
    </div>

    <div class="skin-preview-shell">
      {#if selectedSkin}
        <SkinCharacterPreview
          skinUrl={selectedSkin.skinUrl}
          armType={selectedSkin.armType}
          initialYaw={150}
          scale={1.02}
        />
      {:else}
        <div class="skin-empty-preview">Выберите или загрузите скин</div>
      {/if}
    </div>

    <div class="skin-preview-meta">Зажми и потяни, чтобы вращать модель</div>
  </aside>
</section>

{#if contextMenuSkinId}
  <div
    class="skin-context-menu"
    style={`left:${contextMenuX}px;top:${contextMenuY}px;`}
    role="menu"
    tabindex="-1"
    on:pointerdown|stopPropagation
  >
    <button
      type="button"
      class="skin-context-menu-delete"
      role="menuitem"
      on:click={openDeleteConfirmFromContextMenu}
    >
      Удалить
    </button>
  </div>
{/if}

{#if showDeleteConfirm}
  <div
    class="skin-delete-modal-backdrop"
    role="presentation"
    on:click={cancelDeleteSkin}
  >
    <div
      class="skin-delete-modal"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Подтверждение удаления скина"
      on:pointerdown|stopPropagation
    >
      <p>Вы точно уверены, что хотите удалить скин из списка?</p>
      <div class="skin-delete-modal-actions">
        <button type="button" class="skin-delete-cancel" on:click={cancelDeleteSkin}
          >Отмена</button
        >
        <button type="button" class="skin-delete-confirm" on:click={confirmDeleteSkin}
          >Удалить</button
        >
      </div>
    </div>
  </div>
{/if}

<style>
  .skin-studio {
    display: grid;
    grid-template-columns: minmax(300px, 0.92fr) minmax(420px, 1.08fr);
    gap: clamp(12px, 1.1vw, 18px);
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .skin-library,
  .skin-preview-panel {
    min-height: 0;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: radial-gradient(
        circle at top left,
        color-mix(in srgb, var(--accent) 14%, transparent),
        transparent 42%
      ),
      linear-gradient(
        180deg,
        color-mix(in srgb, var(--surface-main) 84%, var(--surface-alt) 16%),
        var(--surface-main)
      );
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.02);
  }

  .skin-library {
    padding: 18px;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 14px;
    overflow: hidden;
  }

  .skin-library-head h2,
  .skin-preview-head h3 {
    margin: 0;
    font-size: clamp(1.45rem, 2.2vw, 2rem);
    letter-spacing: -0.02em;
  }

  .skin-library-head p {
    margin: 6px 0 0;
    color: var(--text-muted);
    font-size: 0.88rem;
  }

  .skin-file-input {
    display: none;
  }

  .skin-grid {
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable both-edges;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    justify-content: stretch;
    gap: 16px;
    align-content: start;
    padding: 4px 8px 10px 2px;
  }

  .skin-grid::-webkit-scrollbar {
    width: 10px;
  }

  .skin-grid::-webkit-scrollbar-thumb {
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 44%, transparent);
    border: 2px solid color-mix(in srgb, var(--surface-main) 80%, transparent);
  }

  .skin-grid::-webkit-scrollbar-track {
    background: transparent;
  }

  .skin-card {
    width: min(100%, 220px);
    justify-self: center;
    min-height: clamp(146px, 22vh, 188px);
    border-radius: 16px;
    border: 1px solid color-mix(in srgb, var(--line) 70%, transparent);
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--surface-elevated) 76%, var(--surface-alt) 24%),
      color-mix(in srgb, var(--surface-main) 90%, var(--surface-alt) 10%)
    );
    color: var(--text-main);
    padding: 12px;
    cursor: pointer;
    transform-origin: center center;
    will-change: transform;
    transition:
      transform 0.16s ease,
      border-color 0.16s ease,
      box-shadow 0.16s ease,
      background-color 0.16s ease;
  }

  .skin-card:hover {
    transform: translateY(-2px) scale(1.015);
    border-color: color-mix(in srgb, var(--accent) 66%, transparent);
    box-shadow: 0 10px 22px rgba(0, 0, 0, 0.2);
  }

  .skin-card:active {
    transform: scale(0.99);
  }

  .add-skin-card {
    display: grid;
    place-items: center;
    gap: 10px;
    border-style: dashed;
    color: var(--text-soft);
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--surface-elevated) 70%, transparent),
      color-mix(in srgb, var(--surface-alt) 84%, transparent)
    );
  }

  .add-skin-plus {
    font-size: 3rem;
    font-weight: 300;
    line-height: 1;
    color: var(--text-main);
  }

  .saved-skin-card {
    display: grid;
    grid-template-rows: auto auto;
    align-content: start;
    gap: 10px;
    overflow: hidden;
  }

  .saved-skin-card.selected {
    border-color: color-mix(in srgb, var(--accent) 78%, transparent);
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 36%, var(--surface-elevated) 64%),
      color-mix(in srgb, var(--accent) 22%, var(--surface-main) 78%)
    );
    box-shadow:
      inset 0 0 0 1px color-mix(in srgb, var(--accent) 30%, transparent),
      0 14px 30px rgba(0, 0, 0, 0.24);
  }

  .skin-card-preview {
    min-height: 0;
    aspect-ratio: 16 / 11;
    border-radius: 12px;
    overflow: hidden;
    background: radial-gradient(
        circle at 50% 80%,
        color-mix(in srgb, var(--surface-main) 60%, transparent),
        transparent 32%
      ),
      color-mix(in srgb, var(--surface-main) 60%, transparent);
  }

  .skin-card-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    color: var(--text-main);
    font-size: 0.86rem;
    font-weight: 700;
  }

  .skin-card-footer small {
    color: var(--text-muted);
    font-size: 0.74rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .skin-preview-panel {
    padding: 20px;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    gap: 14px;
    overflow: hidden;
  }

  .skin-preview-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .skin-arm-chip {
    display: inline-flex;
    align-items: center;
    min-height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface-elevated) 80%, transparent);
    color: var(--text-soft);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .skin-preview-shell {
    min-height: clamp(260px, 45vh, 620px);
    border-radius: 18px;
    border: 1px solid color-mix(in srgb, var(--line) 70%, transparent);
    background: radial-gradient(
        circle at 50% 78%,
        color-mix(in srgb, var(--surface-main) 64%, transparent),
        transparent 28%
      ),
      linear-gradient(
        180deg,
        color-mix(in srgb, var(--surface-alt) 88%, var(--surface-main) 12%),
        color-mix(in srgb, var(--surface-main) 94%, var(--surface-alt) 6%)
      );
    overflow: hidden;
  }

  .skin-empty-preview {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--text-muted);
    font-size: 0.95rem;
  }

  .skin-preview-meta {
    color: var(--text-muted);
    font-size: 0.88rem;
    text-align: center;
  }

  .skin-context-menu {
    position: fixed;
    z-index: 40;
    min-width: 160px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--surface-main);
    box-shadow: 0 12px 28px rgba(0, 0, 0, 0.26);
    padding: 6px;
  }

  .skin-context-menu-delete {
    width: 100%;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: #ff7a7a;
    font: inherit;
    font-size: 0.9rem;
    font-weight: 700;
    text-align: left;
    padding: 9px 10px;
    cursor: pointer;
    transition:
      border-color 0.14s ease,
      background-color 0.14s ease,
      transform 0.14s ease;
  }

  .skin-context-menu-delete:hover {
    border-color: color-mix(in srgb, #ff8d8d 66%, transparent);
    background: color-mix(in srgb, #ff5d5d 16%, transparent);
    transform: translateX(1px);
  }

  .skin-delete-modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 18px;
    background: rgba(6, 10, 16, 0.56);
    backdrop-filter: blur(3px);
  }

  .skin-delete-modal {
    width: min(420px, 92vw);
    border-radius: 14px;
    border: 1px solid var(--line);
    background: var(--surface-main);
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.32);
    padding: 16px;
    display: grid;
    gap: 14px;
  }

  .skin-delete-modal p {
    margin: 0;
    color: var(--text-main);
    font-size: 0.98rem;
    line-height: 1.35;
  }

  .skin-delete-modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  .skin-delete-cancel,
  .skin-delete-confirm {
    border-radius: 10px;
    padding: 8px 12px;
    font: inherit;
    font-size: 0.88rem;
    font-weight: 700;
    cursor: pointer;
    transition:
      transform 0.14s ease,
      border-color 0.14s ease,
      box-shadow 0.14s ease,
      background-color 0.14s ease;
  }

  .skin-delete-cancel {
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
  }

  .skin-delete-cancel:hover {
    border-color: color-mix(in srgb, var(--accent) 68%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 32%, transparent);
  }

  .skin-delete-confirm {
    border: 1px solid rgba(255, 82, 82, 0.6);
    background: color-mix(in srgb, #ff5d5d 20%, transparent);
    color: #ff7b7b;
  }

  .skin-delete-confirm:hover {
    border-color: rgba(255, 102, 102, 0.9);
    box-shadow: 0 0 0 1px rgba(255, 82, 82, 0.32);
    background: color-mix(in srgb, #ff5d5d 28%, transparent);
  }

  @media (max-width: 1320px) {
    .skin-studio {
      grid-template-columns: minmax(280px, 0.95fr) minmax(360px, 1.05fr);
    }
  }

  @media (max-width: 1024px) {
    .skin-studio {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
      overflow-y: auto;
      overflow-x: hidden;
    }

    .skin-preview-panel {
      grid-template-rows: auto minmax(260px, 1fr) auto;
    }
  }

  @media (max-height: 820px) {
    .skin-library {
      padding: 14px;
    }

    .skin-preview-panel {
      padding: 14px;
      grid-template-rows: auto minmax(210px, 1fr) auto;
      gap: 10px;
    }
  }

  @media (max-width: 760px) {
    .skin-grid {
      grid-template-columns: 1fr;
    }
  }

  @media (min-width: 1700px) {
    .skin-grid {
      grid-template-columns: repeat(auto-fill, minmax(220px, 220px));
      justify-content: start;
    }

    .skin-card {
      width: 220px;
      justify-self: auto;
    }
  }
</style>
