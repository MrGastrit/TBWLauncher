<script lang="ts">
  type AccountState = {
    nickname: string;
    emailOrLogin: string;
  };

  type SaveAccountPayload = {
    nickname: string;
    currentPassword: string;
    nextPassword: string;
  };

  type AccountChangeStatus = {
    role: string;
    nicknameChangeDate?: string;
    passwordChangeDate?: string;
    nicknameCooldownDays: number;
    passwordCooldownDays: number;
    nicknameRemainingSeconds: number;
    passwordRemainingSeconds: number;
    canChangeNickname: boolean;
    canChangePassword: boolean;
  };

  export let account: AccountState;
  export let onBack: () => void;
  export let onSave: (payload: SaveAccountPayload) => Promise<void | string>;
  export let changeStatus: AccountChangeStatus | null = null;
  export let statusLoading = false;

  let nextNickname = account.nickname;
  let currentPassword = "";
  let nextPassword = "";
  let infoMessage = "";
  let infoType: "success" | "error" = "success";
  let isSaving = false;

  function extractErrorMessage(error: unknown): string {
    if (error instanceof Error && error.message) {
      return error.message;
    }

    if (typeof error === "string" && error.trim()) {
      return error;
    }

    if (typeof error === "object" && error !== null) {
      const maybeMessage = (error as { message?: unknown }).message;
      if (typeof maybeMessage === "string" && maybeMessage.trim()) {
        return maybeMessage;
      }
    }

    return "Не удалось сохранить изменения.";
  }

  function formatChangeDate(value?: string): string {
    if (!value) {
      return "—";
    }

    const parsed = new Date(value);
    if (Number.isNaN(parsed.getTime())) {
      return value;
    }

    return parsed.toLocaleString("ru-RU", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function formatRemaining(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds <= 0) {
      return "0 ч";
    }

    if (seconds < 24 * 60 * 60) {
      const hours = Math.max(1, Math.ceil(seconds / (60 * 60)));
      return `${hours} ч`;
    }

    const days = Math.max(1, Math.ceil(seconds / (24 * 60 * 60)));
    return `${days} д`;
  }

  function formatCooldownRule(days: number): string {
    if (days <= 0) {
      return "без ограничений";
    }

    return `раз в ${days} д.`;
  }

  function nicknameCooldownText(): string {
    if (changeStatus) {
      if (changeStatus.canChangeNickname) {
        return "Можно изменить сейчас";
      }

      return `До следующей смены: ${formatRemaining(changeStatus.nicknameRemainingSeconds)}`;
    }

    return statusLoading ? "Загрузка..." : "Нет данных";
  }

  function passwordCooldownText(): string {
    if (changeStatus) {
      if (changeStatus.canChangePassword) {
        return "Можно изменить сейчас";
      }

      return `До следующей смены: ${formatRemaining(changeStatus.passwordRemainingSeconds)}`;
    }

    return statusLoading ? "Загрузка..." : "Нет данных";
  }

  async function saveAccountSettings(): Promise<void> {
    if (isSaving) {
      return;
    }

    isSaving = true;
    infoMessage = "";

    try {
      const message = await onSave({
        nickname: nextNickname.trim(),
        currentPassword,
        nextPassword,
      });

      infoType = "success";
      infoMessage = message ?? "Настройки аккаунта сохранены.";
      currentPassword = "";
      nextPassword = "";
    } catch (error) {
      infoType = "error";
      infoMessage = extractErrorMessage(error);
    } finally {
      isSaving = false;
    }
  }
</script>

<section class="panel" aria-label="Настройки аккаунта">
  <header class="panel-head">
    <h2>Настройки аккаунта</h2>
    <button
      type="button"
      class="back"
      on:click={onBack}
      aria-label="Вернуться в главное меню"
    >
      ↩
    </button>
  </header>

  <div class="form-grid">
    <label for="nickname">Ник</label>
    <div class="field-row">
      <input
        id="nickname"
        class="account-input"
        type="text"
        bind:value={nextNickname}
        maxlength="24"
      />
      <div class="change-meta">
        <span>Дата смены: {formatChangeDate(changeStatus?.nicknameChangeDate)}</span>
        <span>{nicknameCooldownText()}</span>
        <span>Лимит: {formatCooldownRule(changeStatus?.nicknameCooldownDays ?? 0)}</span>
      </div>
    </div>

    <label for="password-current">Текущий пароль</label>
    <div class="field-row field-row-single">
      <input
        id="password-current"
        name="current-password"
        class="account-input"
        type="password"
        bind:value={currentPassword}
        autocomplete="off"
        disabled={isSaving}
      />
    </div>

    <label for="password-new">Новый пароль</label>
    <div class="field-row">
      <input
        id="password-new"
        name="new-password"
        class="account-input"
        type="password"
        bind:value={nextPassword}
        autocomplete="off"
        disabled={isSaving}
      />
      <div class="change-meta">
        <span>Дата смены: {formatChangeDate(changeStatus?.passwordChangeDate)}</span>
        <span>{passwordCooldownText()}</span>
        <span>Лимит: {formatCooldownRule(changeStatus?.passwordCooldownDays ?? 0)}</span>
      </div>
    </div>
  </div>

  {#if infoMessage}
    <p class={infoType === "error" ? "status error" : "status success"}>
      {infoMessage}
    </p>
  {/if}

  <button type="button" class="save" on:click={saveAccountSettings} disabled={isSaving}>
    {#if isSaving}
      Сохранение...
    {:else}
      Сохранить изменения
    {/if}
  </button>
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

  .form-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 10px;
    align-items: center;
  }

  label {
    color: var(--text-soft);
    font-size: 0.9rem;
  }

  input {
    border-radius: 12px;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    font: inherit;
    padding: 11px 12px;
  }

  .field-row {
    display: grid;
    grid-template-columns: minmax(0, 360px) minmax(0, 1fr);
    align-items: center;
    gap: 12px;
  }

  .field-row-single {
    grid-template-columns: minmax(0, 360px);
  }

  .account-input {
    width: 100%;
  }

  .change-meta {
    display: grid;
    gap: 3px;
    color: var(--text-muted);
    font-size: 0.8rem;
    line-height: 1.25;
  }

  .status {
    margin: 0;
    font-size: 0.9rem;
  }

  .success {
    color: #95e3af;
  }

  .error {
    color: #ff8b97;
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

  .save:disabled {
    cursor: not-allowed;
    opacity: 0.75;
    transform: none;
    box-shadow: none;
    filter: none;
  }

  @media (max-width: 980px) {
    .field-row {
      grid-template-columns: 1fr;
    }
  }
</style>
