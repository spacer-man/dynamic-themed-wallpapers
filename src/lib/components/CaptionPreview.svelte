<script lang="ts">
  import type { CaptionAlign } from '../types';

  let {
    image,
    y,
    align,
    enabled,
  }: { image: string | null; y: number; align: CaptionAlign; enabled: boolean } = $props();

  // Home screen icons live roughly here; shown as faint tiles so the user sees what could cover the text.
  const rows = [0, 1, 2, 3];
  const items = $derived(align === 'left' ? 'flex-start' : align === 'right' ? 'flex-end' : 'center');
</script>

<div class="phone" aria-hidden="true" style:background-image={image ? `url("${image}")` : undefined}>
  <div class="icons">
    {#each rows as r (r)}
      <div class="row"><i></i><i></i><i></i><i></i></div>
    {/each}
  </div>
  {#if enabled}
    <div class="caption" style:top="{y}%" style:align-items={items}>
      <b></b>
      <u></u>
    </div>
  {/if}
</div>

<style>
  .phone {
    position: relative;
    width: 108px;
    aspect-ratio: 9 / 19.5;
    flex: none;
    border-radius: var(--radius-sm);
    background-color: color-mix(in srgb, var(--text) 14%, var(--surface));
    background-size: cover;
    background-position: center;
    overflow: hidden;
    border: 2px solid color-mix(in srgb, var(--text) 25%, transparent);
  }

  .icons {
    position: absolute;
    inset: auto 8px 16px 8px;
    display: grid;
    gap: var(--space-2);
  }

  .row {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--space-2);
  }

  i {
    aspect-ratio: 1;
    border-radius: 6px;
    background: color-mix(in srgb, #fff 55%, transparent);
  }

  .caption {
    position: absolute;
    left: 8px;
    right: 8px;
    transform: translateY(-50%);
    display: flex;
    flex-direction: column;
    gap: 3px;
    transition: top 120ms;
  }

  b,
  u {
    display: block;
    height: 6px;
    border-radius: 3px;
    background: #fff;
    box-shadow: 0 0 0 1px color-mix(in srgb, #000 55%, transparent);
    text-decoration: none;
  }

  b {
    width: 70%;
  }

  u {
    width: 45%;
    height: 4px;
    opacity: 0.8;
  }

  @media (prefers-reduced-motion: reduce) {
    .caption {
      transition: none;
    }
  }
</style>
