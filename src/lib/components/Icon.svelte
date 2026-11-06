<script lang="ts">
  type Name =
    | 'today'
    | 'place'
    | 'look'
    | 'schedule'
    | 'history'
    | 'heart'
    | 'close'
    | 'refresh'
    | 'info'
    | 'search'
    | 'edit'
    | 'trash'
    | 'check';

  let { name, size = 24, filled = false }: { name: Name; size?: number; filled?: boolean } = $props();

  // 24×24 stroke icons, drawn for this project.
  const paths: Record<Name, string> = {
    today: 'M4 5h16v14H4z M4 16l4.5-4.5 4 4 3-3L20 17',
    place: 'M12 21s-6.5-5.4-6.5-10.5a6.5 6.5 0 0 1 13 0C18.5 15.6 12 21 12 21z M12 8.5a2 2 0 1 0 0 4 2 2 0 0 0 0-4z',
    look: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M12 3v18',
    schedule: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M12 7.5V12l3 2',
    history: 'M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z',
    heart: 'M12 20s-7.5-4.6-7.5-10.2A4.2 4.2 0 0 1 12 7.4a4.2 4.2 0 0 1 7.5 2.4C19.5 15.4 12 20 12 20z',
    close: 'M6 6l12 12 M18 6L6 18',
    refresh: 'M19.5 12a7.5 7.5 0 1 1-2.2-5.3 M19.5 4.5v4.8h-4.8',
    info: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M12 11v5.5 M12 7.6v.01',
    edit: 'M4 20h4L19 9l-4-4L4 16z M13.5 6.5l4 4',
    trash: 'M5 7h14 M9 7V4h6v3 M7 7l1 13h8l1-13',
    check: 'M5 12.5l4.5 4.5L19 7',
    search: 'M10.5 4a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13z M20 20l-4.8-4.8',
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={filled ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name]} />
</svg>
