<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements'

  export let id = ''
  export let label = ''
  export let placeholder = ''
  export let autocomplete: HTMLInputAttributes['autocomplete'] = 'current-password'
  export let value = ''
  export let compact = false

  let isVisible = false

  $: inputType = isVisible ? 'text' : 'password'
  $: eyeLabel = isVisible ? 'Скрыть пароль' : 'Показать пароль'
</script>

<label for={id} class="field-label">{label}</label>
<div class="password-wrapper" class:compact>
  <input
    id={id}
    class="password-input"
    type={inputType}
    bind:value
    placeholder={placeholder}
    autocomplete={autocomplete}
  />

  <button type="button" class="eye-toggle" on:click={() => (isVisible = !isVisible)} aria-label={eyeLabel} title={eyeLabel}>
    {#if isVisible}
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M3 5l16 16" />
        <path d="M10.6 10.6a2 2 0 0 0 2.8 2.8" />
        <path d="M9.9 5.1A10.7 10.7 0 0 1 12 5c6.5 0 10 7 10 7a16 16 0 0 1-3.3 4.4" />
        <path d="M6.3 8.3A16.9 16.9 0 0 0 2 12s3.5 7 10 7a10.4 10.4 0 0 0 3.2-.5" />
      </svg>
    {:else}
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z" />
        <circle cx="12" cy="12" r="3" />
      </svg>
    {/if}
  </button>
</div>

<style>
  .field-label {
    color: #bfd0ef;
    font-size: 0.9rem;
  }

  .password-wrapper {
    position: relative;
    width: 100%;
  }

  .password-input {
    width: 100%;
    box-sizing: border-box;
    border-radius: 12px;
    border: 1px solid rgba(111, 130, 162, 0.34);
    background: rgba(8, 11, 17, 0.74);
    color: #eff5ff;
    padding: 12px 42px 12px 14px;
    font: inherit;
    transition: border-color 0.2s, box-shadow 0.2s;
  }

  .password-input:focus {
    outline: none;
    border-color: rgba(111, 177, 255, 0.9);
    box-shadow: 0 0 0 3px rgba(86, 153, 255, 0.25);
  }

  .password-wrapper.compact {
    width: calc(100% - 10px);
    margin: 0 auto;
  }

  .password-wrapper.compact .password-input {
    padding-top: 10px;
    padding-bottom: 10px;
  }

  .eye-toggle {
    position: absolute;
    top: 50%;
    right: 8px;
    transform: translateY(-50%);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: #b9c9e8;
    cursor: pointer;
    transition: background-color 0.16s ease;
  }

  .eye-toggle:hover {
    background: rgba(148, 176, 224, 0.15);
  }

  .eye-toggle:focus-visible {
    outline: 2px solid rgba(111, 177, 255, 0.9);
    outline-offset: 1px;
  }

  .eye-toggle svg {
    width: 17px;
    height: 17px;
    stroke: currentColor;
    fill: none;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .eye-toggle circle {
    fill: none;
  }
</style>
