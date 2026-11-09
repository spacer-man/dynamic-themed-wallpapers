<script lang="ts" generics="T extends string">
  let {
    options,
    value,
    disabled = false,
    onchange,
  }: {
    options: { value: T; label: string }[];
    value: T;
    disabled?: boolean;
    onchange: (v: T) => void;
  } = $props();
</script>

<div class="seg" role="radiogroup" class:disabled>
  {#each options as o (o.value)}
    <button
      role="radio"
      aria-checked={value === o.value}
      class:on={value === o.value}
      {disabled}
      onclick={() => onchange(o.value)}>{o.label}</button
    >
  {/each}
</div>

<style>
  .seg {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 1fr;
    gap: 2px;
    padding: 3px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }

  .seg.disabled {
    opacity: 0.5;
  }

  button {
    min-height: 44px;
    border: 0;
    border-radius: 999px;
    background: transparent;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--muted);
    transition: background-color 200ms, color 200ms;
  }

  button.on {
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 1px 3px color-mix(in srgb, #000 22%, transparent);
  }
</style>
