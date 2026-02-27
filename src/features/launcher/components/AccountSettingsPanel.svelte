<script lang="ts">
  type AccountState = {
    nickname: string;
    emailOrLogin: string;
  };

  export let account: AccountState;
  export let onBack: () => void;
  export let onNicknameSave: (nickname: string) => void;

  let nextNickname = account.nickname;
  let currentPassword = "";
  let nextPassword = "";
  let infoMessage = "";

  function saveAccountSettings(): void {
    const normalizedNickname = nextNickname.trim();

    if (normalizedNickname) {
      onNicknameSave(normalizedNickname);
    }

    infoMessage = "Настройки аккаунта сохранены.";
  }
</script>

<section class="panel" aria-label="Настройки аккаунта">
  <header class="panel-head">
    <h2>Настройки аккаунта</h2>
    <button
      type="button"
      class="back"
      on:click={onBack}
      aria-label="Вернуться в главное меню">↩</button
    >
  </header>

  <div class="form-grid">
    <label for="nickname">Ник</label>
    <input
      id="nickname"
      class="account-input"
      type="text"
      bind:value={nextNickname}
      maxlength="24"
    />

    <label for="password-current">Текущий пароль</label>
    <input
      id="password-current"
      class="account-input"
      type="password"
      bind:value={currentPassword}
      autocomplete="current-password"
    />

    <label for="password-new">Новый пароль</label>
    <input
      id="password-new"
      class="account-input"
      type="password"
      bind:value={nextPassword}
      autocomplete="new-password"
    />

  </div>

  {#if infoMessage}
    <p class="success">{infoMessage}</p>
  {/if}

  <button type="button" class="save" on:click={saveAccountSettings}
    >Сохранить изменения</button
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

  .form-grid {
    display: grid;
    grid-template-columns: 360px 1fr;
    column-gap: 16px;
    gap: 10px;
    align-items: center;
  }

  label {
    grid-column: 1;
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

  .account-input {
    grid-column: 1;
    width: 100%;
    max-width: 360px;
    justify-self: start;
  }

  @media (max-width: 860px) {
    .form-grid {
      grid-template-columns: 1fr;
      column-gap: 0;
    }
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
