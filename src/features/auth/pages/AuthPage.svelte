<script lang="ts">
  import AuthForm from '../components/AuthForm.svelte'
  import { createEventDispatcher } from 'svelte'

  type AuthSuccessPayload = {
    id: string
    nickname: string
    emailOrLogin: string
    skinUrl?: string
    role: string
  }

  const dispatch = createEventDispatcher<{ authSuccess: AuthSuccessPayload }>()

  function relayAuthSuccess(event: CustomEvent<AuthSuccessPayload>): void {
    dispatch('authSuccess', event.detail)
  }
</script>

<main class="auth-page">
  <div class="orb orb-left" aria-hidden="true"></div>
  <div class="orb orb-right" aria-hidden="true"></div>
  <AuthForm on:authSuccess={relayAuthSuccess} />
</main>

<style>
  .auth-page {
    position: relative;
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 28px 14px;
    overflow: hidden;
  }

  .orb {
    position: absolute;
    border-radius: 50%;
    filter: blur(70px);
    opacity: 0.45;
    pointer-events: none;
  }

  .orb-left {
    width: 340px;
    height: 340px;
    background: #2ca0a8;
    left: -110px;
    top: -70px;
  }

  .orb-right {
    width: 300px;
    height: 300px;
    background: #37509a;
    right: -80px;
    bottom: -80px;
  }
</style>
