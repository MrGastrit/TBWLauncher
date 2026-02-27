<script lang="ts">
  import { onMount } from 'svelte'
  import AuthPage from './features/auth/pages/AuthPage.svelte'
  import MainMenuPage from './features/launcher/pages/MainMenuPage.svelte'
  import { clearSession, restoreSession } from './features/auth/services/auth-service'

  type SessionUser = {
    nickname: string
    emailOrLogin: string
  }

  let isAuthenticated = false
  let sessionUser: SessionUser = {
    nickname: 'Player',
    emailOrLogin: ''
  }

  onMount(() => {
    const preventContextMenu = (event: MouseEvent): void => {
      event.preventDefault()
    }
    window.addEventListener('contextmenu', preventContextMenu)

    const restoredSession = restoreSession()
    if (!restoredSession) {
      return () => {
        window.removeEventListener('contextmenu', preventContextMenu)
      }
    }

    sessionUser = {
      nickname: restoredSession.user.nickname,
      emailOrLogin: restoredSession.user.email
    }
    isAuthenticated = true

    return () => {
      window.removeEventListener('contextmenu', preventContextMenu)
    }
  })

  function handleAuthSuccess(event: CustomEvent<SessionUser>): void {
    sessionUser = event.detail
    isAuthenticated = true
  }

  function handleSignOut(): void {
    clearSession()
    isAuthenticated = false
    sessionUser = {
      nickname: 'Player',
      emailOrLogin: ''
    }
  }
</script>

{#if isAuthenticated}
  <MainMenuPage user={sessionUser} on:signOut={handleSignOut} />
{:else}
  <AuthPage on:authSuccess={handleAuthSuccess} />
{/if}
