<script lang="ts">
  import { onMount } from "svelte";
  import type { AdminUserSummary } from "../../auth/models/auth";
  import {
    adminListUsers,
    adminSetUserBanned,
    adminSetUserRole,
  } from "../../auth/services/auth-service";

  export let actorUserId: string;
  export let actorIdentity: string;
  export let actorRole: string;
  export let onBack: () => void;

  let users: AdminUserSummary[] = [];
  let searchValue = "";
  let showOnlyBanned = false;
  let loading = false;
  let statusMessage = "";
  let statusType: "success" | "error" | "" = "";
  let actionInFlightByUserId: Record<string, boolean> = {};
  let roleDraftByUserId: Record<string, string> = {};

  const actorRoleNormalized = (actorRole || "").trim().toLowerCase();
  const actorIsAdmin = actorRoleNormalized === "admin";
  const allowedRoleOptions = actorIsAdmin
    ? ["user", "vip", "tech", "admin"]
    : ["user", "vip"];

  $: filteredUsers = users.filter((user) => {
    if (showOnlyBanned && !user.banned) {
      return false;
    }

    return true;
  });

  $: totalUsersCount = users.length;
  $: bannedUsersCount = users.filter((user) => user.banned).length;
  $: staffUsersCount = users.filter((user) => {
    const normalizedRole = user.role.trim().toLowerCase();
    return normalizedRole === "admin" || normalizedRole === "tech";
  }).length;

  onMount(() => {
    void refreshUsers();
  });

  async function refreshUsers(): Promise<void> {
    loading = true;
    clearStatus();

    try {
      users = await adminListUsers(actorUserId, actorIdentity, searchValue);
      roleDraftByUserId = Object.fromEntries(
        users.map((user) => [user.id, user.role]),
      );
    } catch (error) {
      setError(extractErrorMessage(error));
    } finally {
      loading = false;
    }
  }

  function canManageUser(user: AdminUserSummary): boolean {
    if (user.id === actorUserId) {
      return false;
    }

    if (actorIsAdmin) {
      return true;
    }

    const normalizedRole = user.role.trim().toLowerCase();
    return normalizedRole !== "admin" && normalizedRole !== "tech";
  }

  function canAssignRole(role: string): boolean {
    if (actorIsAdmin) {
      return true;
    }

    const normalizedRole = role.trim().toLowerCase();
    return normalizedRole === "user" || normalizedRole === "vip";
  }

  async function handleApplyRole(user: AdminUserSummary): Promise<void> {
    if (!canManageUser(user)) {
      return;
    }

    const nextRole = (roleDraftByUserId[user.id] || user.role).trim().toLowerCase();
    if (!nextRole || !canAssignRole(nextRole)) {
      setError("Недопустимая роль для назначения.");
      return;
    }

    if (nextRole === user.role.trim().toLowerCase()) {
      return;
    }

    setActionInFlight(user.id, true);
    clearStatus();

    try {
      await adminSetUserRole(actorUserId, actorIdentity, user.nickname, nextRole);
      users = users.map((entry) =>
        entry.id === user.id
          ? {
              ...entry,
              role: nextRole,
            }
          : entry,
      );
      setSuccess(`Роль пользователя ${user.nickname} обновлена.`);
    } catch (error) {
      setError(extractErrorMessage(error));
      roleDraftByUserId = {
        ...roleDraftByUserId,
        [user.id]: user.role,
      };
    } finally {
      setActionInFlight(user.id, false);
    }
  }

  async function handleToggleBan(user: AdminUserSummary): Promise<void> {
    if (!canManageUser(user)) {
      return;
    }

    const nextBanState = !user.banned;
    setActionInFlight(user.id, true);
    clearStatus();

    try {
      await adminSetUserBanned(actorUserId, actorIdentity, user.nickname, nextBanState);
      users = users.map((entry) =>
        entry.id === user.id
          ? {
              ...entry,
              banned: nextBanState,
            }
          : entry,
      );
      setSuccess(
        nextBanState
          ? `Пользователь ${user.nickname} заблокирован.`
          : `Пользователь ${user.nickname} разблокирован.`,
      );
    } catch (error) {
      setError(extractErrorMessage(error));
    } finally {
      setActionInFlight(user.id, false);
    }
  }

  function setActionInFlight(userId: string, value: boolean): void {
    actionInFlightByUserId = {
      ...actionInFlightByUserId,
      [userId]: value,
    };
  }

  function onRoleDraftChange(userId: string, value: string): void {
    roleDraftByUserId = {
      ...roleDraftByUserId,
      [userId]: value,
    };
  }

  function extractErrorMessage(error: unknown): string {
    if (error instanceof Error && error.message.trim()) {
      return error.message;
    }

    if (typeof error === "string" && error.trim()) {
      return error;
    }

    return "Не удалось выполнить действие в админ-панели.";
  }

  function setSuccess(message: string): void {
    statusType = "success";
    statusMessage = message;
  }

  function setError(message: string): void {
    statusType = "error";
    statusMessage = message;
  }

  function clearStatus(): void {
    statusType = "";
    statusMessage = "";
  }
</script>

<section class="admin-panel" aria-label="Админ-панель">
  <header class="admin-head">
    <h2>Админ-панель</h2>
    <button type="button" class="back-btn" on:click={onBack} aria-label="Вернуться">↩</button>
  </header>

  <div class="stats-row" aria-label="Сводка пользователей">
    <div class="stat-card">
      <div class="stat-value">{totalUsersCount}</div>
      <div class="stat-label">Всего</div>
    </div>
    <div class="stat-card banned">
      <div class="stat-value">{bannedUsersCount}</div>
      <div class="stat-label">Забанены</div>
    </div>
    <div class="stat-card staff">
      <div class="stat-value">{staffUsersCount}</div>
      <div class="stat-label">Admin/Tech</div>
    </div>
  </div>

  <div class="toolbar">
    <label class="search-field" aria-label="Поиск пользователя">
      <input
        type="text"
        bind:value={searchValue}
        placeholder="Поиск по нику или почте"
        on:keydown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            void refreshUsers();
          }
        }}
      />
    </label>

    <label class="checkbox-row">
      <input type="checkbox" bind:checked={showOnlyBanned} />
      Только забаненные
    </label>

    <button type="button" class="refresh-btn" on:click={refreshUsers} disabled={loading}>
      {loading ? "Обновление..." : "Обновить"}
    </button>
  </div>

  {#if statusMessage}
    <p class="status {statusType}">{statusMessage}</p>
  {/if}

  <div class="table-wrap">
    {#if loading && users.length === 0}
      <p class="empty">Загрузка пользователей...</p>
    {:else if filteredUsers.length === 0}
      <p class="empty">Пользователи не найдены.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Ник</th>
            <th>Email</th>
            <th>Роль</th>
            <th>Статус</th>
            <th>Действия</th>
          </tr>
        </thead>
        <tbody>
          {#each filteredUsers as user (user.id)}
            <tr>
              <td class="nickname">{user.nickname}</td>
              <td class="email">{user.email}</td>
              <td>
                <select
                  value={roleDraftByUserId[user.id] ?? user.role}
                  disabled={!canManageUser(user) || actionInFlightByUserId[user.id]}
                  on:change={(event) =>
                    onRoleDraftChange(user.id, (event.currentTarget as HTMLSelectElement).value)}
                >
                  {#each allowedRoleOptions as roleOption (roleOption)}
                    <option value={roleOption}>{roleOption}</option>
                  {/each}
                </select>
              </td>
              <td>
                <span class="badge" class:is-banned={user.banned}>
                  {user.banned ? "Забанен" : "Активен"}
                </span>
              </td>
              <td>
                <div class="actions">
                  <button
                    type="button"
                    class="apply-btn"
                    on:click={() => handleApplyRole(user)}
                    disabled={!canManageUser(user) || actionInFlightByUserId[user.id]}
                  >
                    Применить роль
                  </button>
                  <button
                    type="button"
                    class="ban-btn"
                    class:unban={user.banned}
                    on:click={() => handleToggleBan(user)}
                    disabled={!canManageUser(user) || actionInFlightByUserId[user.id]}
                  >
                    {user.banned ? "Разбан" : "Бан"}
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</section>

<style>
  .admin-panel {
    display: grid;
    gap: 12px;
    color: var(--text-main);
  }

  .admin-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }

  .admin-head h2 {
    margin: 0;
  }

  .back-btn {
    width: 38px;
    height: 38px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    cursor: pointer;
  }

  .stats-row {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  .stat-card {
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--surface-alt);
    padding: 10px;
    display: grid;
    gap: 2px;
  }

  .stat-card.banned {
    border-color: rgba(255, 96, 96, 0.4);
  }

  .stat-card.staff {
    border-color: rgba(123, 172, 255, 0.4);
  }

  .stat-value {
    font-size: 1.05rem;
    font-weight: 700;
  }

  .stat-label {
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .toolbar {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 10px;
    align-items: center;
  }

  .search-field input {
    width: 100%;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface-alt);
    color: var(--text-main);
    font: inherit;
    padding: 9px 10px;
  }

  .checkbox-row {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
    font-size: 0.86rem;
  }

  .refresh-btn {
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-main);
    font: inherit;
    padding: 9px 11px;
    cursor: pointer;
  }

  .status {
    margin: 0;
    font-size: 0.88rem;
  }

  .status.success {
    color: #8ee0a9;
  }

  .status.error {
    color: #ff9b9b;
  }

  .table-wrap {
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--surface-alt);
    max-height: min(520px, calc(100vh - 280px));
    overflow: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    min-width: 760px;
  }

  th,
  td {
    text-align: left;
    padding: 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 78%, transparent);
    font-size: 0.86rem;
    vertical-align: middle;
  }

  th {
    position: sticky;
    top: 0;
    background: color-mix(in srgb, var(--surface-alt) 85%, #0d1524 15%);
    z-index: 1;
  }

  .nickname {
    font-weight: 700;
  }

  .email {
    color: var(--text-muted);
  }

  select {
    min-width: 110px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--surface-main);
    color: var(--text-main);
    font: inherit;
    padding: 6px 8px;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    border: 1px solid rgba(115, 198, 143, 0.45);
    border-radius: 999px;
    color: #9ee4b5;
    background: rgba(66, 143, 95, 0.14);
    padding: 4px 8px;
    font-size: 0.76rem;
    font-weight: 700;
  }

  .badge.is-banned {
    border-color: rgba(255, 100, 100, 0.45);
    color: #ffb1b1;
    background: rgba(204, 66, 66, 0.16);
  }

  .actions {
    display: inline-flex;
    gap: 6px;
  }

  .apply-btn,
  .ban-btn {
    border-radius: 8px;
    border: 1px solid var(--line);
    background: var(--surface-main);
    color: var(--text-main);
    font: inherit;
    font-size: 0.78rem;
    padding: 6px 8px;
    cursor: pointer;
  }

  .ban-btn {
    border-color: rgba(255, 89, 89, 0.55);
    color: #ff9e9e;
    background: rgba(255, 89, 89, 0.1);
  }

  .ban-btn.unban {
    border-color: rgba(118, 214, 157, 0.52);
    color: #95e2b4;
    background: rgba(118, 214, 157, 0.11);
  }

  .empty {
    margin: 0;
    padding: 16px;
    color: var(--text-muted);
    font-size: 0.9rem;
  }

  button:disabled,
  select:disabled {
    cursor: default;
    opacity: 0.6;
  }

  @media (max-width: 760px) {
    .toolbar {
      grid-template-columns: 1fr;
      align-items: stretch;
    }

    .stats-row {
      grid-template-columns: 1fr;
    }

    .table-wrap {
      max-height: min(520px, calc(100vh - 360px));
    }
  }
</style>
