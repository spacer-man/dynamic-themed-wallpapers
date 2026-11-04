<script lang="ts">
  let {
    checked,
    label,
    hint,
    onchange,
  }: { checked: boolean; label: string; hint?: string; onchange: (v: boolean) => void } = $props();
</script>

<label class="row">
  <span class="text">
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
  <input type="checkbox" role="switch" {checked} onchange={(e) => onchange(e.currentTarget.checked)} />
</label>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    min-height: 44px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  input {
    appearance: none;
    flex: none;
    width: 50px;
    height: 30px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 22%, transparent);
    position: relative;
    transition: background-color 200ms;
    cursor: pointer;
  }

  input::after {
    content: '';
    position: absolute;
    top: 3px;
    left: 3px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px color-mix(in srgb, #000 30%, transparent);
    transition: transform 200ms var(--ease);
  }

  input:checked {
    background: var(--accent);
  }

  input:checked::after {
    transform: translateX(20px);
  }
</style>
