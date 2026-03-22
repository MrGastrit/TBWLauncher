<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte'
  import { fade } from 'svelte/transition'
  import PasswordField from './PasswordField.svelte'
  import { login, register } from '../services/auth-service'
  import { validateLoginForm, validateRegisterForm } from '../validation/auth-validation'

  type AuthMode = 'register' | 'login'

  type AuthSuccessPayload = {
    id: string
    nickname: string
    emailOrLogin: string
    skinUrl?: string
    role: string
  }

  const dispatch = createEventDispatcher<{ authSuccess: AuthSuccessPayload }>()

  const onboardingKey = 'tbwlauncher-auth-onboarded'

  let mode: AuthMode = 'register'

  let registerEmail = ''
  let registerNickname = ''
  let registerPassword = ''
  let registerPasswordRepeat = ''

  let loginIdentity = ''
  let loginPassword = ''

  let statusMessage = ''
  let statusType: 'error' | 'success' | '' = ''
  let isSubmitting = false

  onMount(() => {
    const hasOnboarding = localStorage.getItem(onboardingKey) === 'true'
    mode = hasOnboarding ? 'login' : 'register'
  })

  function switchMode(nextMode: AuthMode): void {
    mode = nextMode
    statusMessage = ''
    statusType = ''
  }

  function setError(message: string): void {
    statusMessage = normalizeErrorMessage(message)
    statusType = 'error'
  }

  function setSuccess(message: string): void {
    statusMessage = message
    statusType = 'success'
  }

  function emitAuthSuccess(
    id: string,
    nickname: string,
    emailOrLogin: string,
    skinUrl?: string,
    role = 'user',
  ): void {
    dispatch('authSuccess', {
      id,
      nickname,
      emailOrLogin,
      skinUrl,
      role,
    })
  }

  function extractErrorMessage(error: unknown): string {
    if (error instanceof Error && error.message) {
      return normalizeErrorMessage(error.message)
    }

    if (typeof error === 'string' && error.trim()) {
      return normalizeErrorMessage(error)
    }

    if (typeof error === 'object' && error !== null) {
      const maybeMessage = (error as { message?: unknown }).message
      if (typeof maybeMessage === 'string' && maybeMessage.trim()) {
        return normalizeErrorMessage(maybeMessage)
      }
    }

    return 'Не удалось выполнить запрос к серверу.'
  }

  function normalizeErrorMessage(message: string): string {
    const normalized = message.trim()
    if (!normalized) {
      return 'Не удалось выполнить запрос к серверу.'
    }

    const lower = normalized.toLowerCase()
    if (lower.includes('email is required')) {
      return 'Введите email.'
    }
    if (lower.includes('nickname must contain at least')) {
      return 'Ник должен содержать минимум 3 символа.'
    }
    if (lower.includes('nickname must not exceed')) {
      return 'Ник не должен превышать 24 символа.'
    }
    if (lower.includes('nickname may contain only')) {
      return 'Ник может содержать только английские буквы, цифры и нижнее подчеркивание (_).'
    }
    if (lower.includes('passwords do not match')) {
      return 'Пароли не совпадают.'
    }
    if (lower.includes('a user with this email already exists')) {
      return 'Пользователь с таким email уже существует.'
    }
    if (lower.includes('this nickname is already taken')) {
      return 'Ник уже занят.'
    }
    if (lower.includes('this account is banned')) {
      return 'Аккаунт заблокирован. Обратитесь к администрации.'
    }
    if (lower.includes('invalid credentials')) {
      return 'Неверный логин или пароль.'
    }

    if (!looksCorrupted(normalized)) {
      return normalized
    }

    if (mode === 'login') {
      return 'Неверный логин или пароль.'
    }

    return 'Произошла ошибка при регистрации. Проверьте введенные данные.'
  }

  function looksCorrupted(message: string): boolean {
    if (message.includes('\uFFFD')) {
      return true
    }

    if (/[ÐÑ]/.test(message)) {
      return true
    }

    let qCount = 0
    for (const ch of message) {
      if (ch === '?') {
        qCount += 1
      }
    }
    if (message.length > 0 && qCount / message.length > 0.2 && qCount >= 3) {
      return true
    }

    let rsCount = 0
    for (const ch of message) {
      if (ch === 'Р' || ch === 'С') {
        rsCount += 1
      }
    }

    return message.length > 0 && rsCount / message.length > 0.22
  }

  async function handleSubmit(event: SubmitEvent): Promise<void> {
    event.preventDefault()

    if (isSubmitting) {
      return
    }

    statusMessage = ''
    statusType = ''
    isSubmitting = true

    try {
      if (mode === 'register') {
        const validationError = validateRegisterForm({
          email: registerEmail,
          nickname: registerNickname,
          password: registerPassword,
          repeatPassword: registerPasswordRepeat
        })

        if (validationError) {
          setError(validationError)
          return
        }

        const result = await register({
          email: registerEmail,
          nickname: registerNickname,
          password: registerPassword,
          repeatPassword: registerPasswordRepeat
        })

        localStorage.setItem(onboardingKey, 'true')
        setSuccess('Аккаунт зарегистрирован!')
        emitAuthSuccess(
          result.user.id,
          result.user.nickname,
          result.user.email,
          result.user.skinUrl,
          result.user.role,
        )
        return
      }

      const validationError = validateLoginForm({
        identity: loginIdentity,
        password: loginPassword
      })

      if (validationError) {
        setError(validationError)
        return
      }

      const result = await login({
        identity: loginIdentity,
        password: loginPassword
      })

      emitAuthSuccess(
        result.user.id,
        result.user.nickname,
        result.user.email,
        result.user.skinUrl,
        result.user.role,
      )
    } catch (error) {
      console.error('Auth request failed:', error)
      const message = extractErrorMessage(error)
      setError(message)
    } finally {
      isSubmitting = false
    }
  }
</script>

<section class="auth-shell" aria-label="Авторизация">
  <div class="auth-card">
    <header class="auth-header">
      <p class="auth-subtitle">TBW Launcher</p>
      <h1>{mode === 'register' ? 'Создание аккаунта' : 'Вход в аккаунт'}</h1>
      <p class="auth-text">
        {mode === 'register'
          ? 'Первый запуск: зарегистрируйся, чтобы продолжить.'
          : 'Введите данные от аккаунта, чтобы начать жоскую игру.'}
      </p>
    </header>

    <div class="mode-switch" role="tablist" aria-label="Форма">
      <button
        type="button"
        role="tab"
        class:active={mode === 'register'}
        aria-selected={mode === 'register'}
        on:click={() => switchMode('register')}
      >
        Регистрация
      </button>
      <button
        type="button"
        role="tab"
        class:active={mode === 'login'}
        aria-selected={mode === 'login'}
        on:click={() => switchMode('login')}
      >
        Вход
      </button>
    </div>

    <form class="auth-form" on:submit={handleSubmit} novalidate>
      {#key mode}
        <div class="mode-panel">
          {#if mode === 'register'}
            <label for="reg-email">Почта</label>
            <input
              class="auth-input"
              id="reg-email"
              type="email"
              bind:value={registerEmail}
              placeholder="you@example.com"
              autocomplete="email"
            />

            <label for="reg-nickname">Ник</label>
            <input
              class="auth-input"
              id="reg-nickname"
              type="text"
              bind:value={registerNickname}
              placeholder="Player123"
              autocomplete="username"
              minlength="3"
              maxlength="24"
              pattern="[A-Za-z0-9_]+"
              title="Только английские буквы, цифры и нижнее подчеркивание (_)."
            />

            <PasswordField
              id="reg-password"
              label="Пароль"
              bind:value={registerPassword}
              autocomplete="new-password"
              placeholder="Введите пароль"
              compact={true}
            />
            <PasswordField
              id="reg-password-repeat"
              label="Повтор пароля"
              bind:value={registerPasswordRepeat}
              autocomplete="new-password"
              placeholder="Повторите пароль"
              compact={true}
            />
          {:else}
            <label for="login-id">Почта или ник</label>
            <input
              class="auth-input"
              id="login-id"
              type="text"
              bind:value={loginIdentity}
              placeholder="you@example.com"
              autocomplete="username"
            />

            <PasswordField
              id="login-password"
              label="Пароль"
              bind:value={loginPassword}
              autocomplete="current-password"
              placeholder="Введите пароль"
              compact={true}
            />
          {/if}
        </div>
      {/key}

      {#if statusMessage}
        <p class="status {statusType}" in:fade={{ duration: 120 }}>
          {statusMessage}
        </p>
      {/if}

      <button class="submit" type="submit" disabled={isSubmitting}>
        {#if isSubmitting}
          Обработка...
        {:else}
          {mode === 'register' ? 'Зарегистрироваться' : 'Войти'}
        {/if}
      </button>
    </form>
  </div>
</section>

<style>
  .auth-shell {
    width: min(580px, 92vw);
  }

  .auth-card {
    border-radius: 24px;
    padding: clamp(20px, 4vw, 34px);
    background: linear-gradient(
      180deg,
      rgba(25, 31, 43, 0.92),
      rgba(17, 21, 30, 0.95)
    );
    border: 1px solid rgba(126, 150, 186, 0.2);
    box-shadow: 0 24px 90px rgba(2, 4, 10, 0.65);
    backdrop-filter: blur(8px);
  }

  .auth-header h1 {
    margin: 6px 0 8px;
    font-size: clamp(1.7rem, 2.2vw, 2.1rem);
    color: #eef4ff;
  }

  .auth-subtitle {
    margin: 0;
    color: #82d6b3;
    text-transform: uppercase;
    font-size: 0.78rem;
    letter-spacing: 0.14em;
    font-weight: 700;
  }

  .auth-text {
    margin: 0;
    color: #a4b0c6;
    font-size: 0.95rem;
  }

  .mode-switch {
    margin-top: 20px;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 6px;
    background: rgba(10, 14, 22, 0.72);
    border-radius: 12px;
    border: 1px solid rgba(111, 130, 162, 0.25);
  }

  .mode-switch button {
    border: none;
    border-radius: 8px;
    padding: 10px 12px;
    font: inherit;
    color: #9eabc4;
    background: transparent;
    cursor: pointer;
    transition: 0.18s ease;
  }

  .mode-switch button.active {
    color: #e7eeff;
    background: rgba(89, 148, 255, 0.22);
  }

  .auth-form {
    margin-top: 18px;
    display: grid;
    gap: 10px;
  }

  .mode-panel {
    display: grid;
    gap: 10px;
  }

  .auth-form label {
    color: #bfd0ef;
    font-size: 0.9rem;
  }

  .auth-input {
    width: 100%;
    box-sizing: border-box;
    border-radius: 12px;
    border: 1px solid rgba(111, 130, 162, 0.34);
    background: rgba(8, 11, 17, 0.74);
    color: #eff5ff;
    padding: 12px 14px;
    font: inherit;
    transition:
      border-color 0.2s,
      box-shadow 0.2s;
  }

  .auth-input:focus {
    outline: none;
    border-color: rgba(111, 177, 255, 0.9);
    box-shadow: 0 0 0 3px rgba(86, 153, 255, 0.25);
  }

  .status {
    margin: 6px 0 0;
    font-size: 0.9rem;
  }

  .status.error {
    color: #ff8b97;
  }

  .status.success {
    color: #98e3b1;
  }

  .submit {
    margin-top: 10px;
    border: none;
    border-radius: 12px;
    padding: 12px 18px;
    font: inherit;
    font-weight: 700;
    color: #111827;
    background: linear-gradient(135deg, #91f3c8, #6bc4ff);
    cursor: pointer;
    transition:
      transform 0.18s,
      filter 0.18s,
      opacity 0.18s;
  }

  .submit:hover {
    transform: translateY(-1px);
    filter: brightness(1.03);
  }

  .submit:active {
    transform: translateY(0);
  }

  .submit:disabled {
    cursor: not-allowed;
    opacity: 0.75;
  }
</style>
