<script lang="ts">
  import { onMount } from 'svelte'
  import AuthPage from './features/auth/pages/AuthPage.svelte'
  import MainMenuPage from './features/launcher/pages/MainMenuPage.svelte'
  import { clearSession, restoreSession } from './features/auth/services/auth-service'

  type SessionUser = {
    id: string
    nickname: string
    emailOrLogin: string
    skinUrl?: string
  }

  let isAuthenticated = false
  let sessionUser: SessionUser = {
    id: '',
    nickname: 'Player',
    emailOrLogin: '',
    skinUrl: ''
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
      id: restoredSession.user.id,
      nickname: restoredSession.user.nickname,
      emailOrLogin: restoredSession.user.email,
      skinUrl: restoredSession.user.skinUrl
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
      id: '',
      nickname: 'Player',
      emailOrLogin: '',
      skinUrl: ''
    }
  }
</script>

{#if isAuthenticated}
  <MainMenuPage user={sessionUser} on:signOut={handleSignOut} />
{:else}
  <AuthPage on:authSuccess={handleAuthSuccess} />
{/if}
