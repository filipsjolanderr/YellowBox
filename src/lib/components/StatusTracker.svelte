<script lang="ts">
  import type { Session } from "$lib/session.svelte";

  let { session } = $props<{ session: Session }>();

  // SVG progress ring: stable rendering that doesn't reorder at 50%.
  const SIZE = 120;
  const STROKE = 12;
  const R = (SIZE - STROKE) / 2;
  const C = 2 * Math.PI * R;

  let totalActive = $derived(session.totalCount);
  let done = $derived(session.completedCount + session.failedCount);
  let ratio = $derived(
    totalActive > 0 ? Math.min(1, Math.max(0, done / totalActive)) : 0,
  );
  let dashOffset = $derived(C * (1 - ratio));
</script>

<div class="flex flex-col items-center justify-center p-2">
  <div class="relative h-[120px] w-[120px]">
    <svg
      viewBox={`0 0 ${SIZE} ${SIZE}`}
      class="h-full w-full drop-shadow-[0_0_22px_rgba(0,0,0,0.12)]"
      aria-label="Overall progress"
      role="img"
    >
      <circle
        cx={SIZE / 2}
        cy={SIZE / 2}
        r={R}
        fill="none"
        stroke="var(--color-secondary)"
        stroke-width={STROKE}
        opacity="0.6"
      />

      <g transform={`rotate(-90 ${SIZE / 2} ${SIZE / 2})`}>
        <circle
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={R}
          fill="none"
          stroke="var(--color-primary)"
          stroke-width={STROKE}
          stroke-linecap="round"
          stroke-dasharray={C}
          stroke-dashoffset={dashOffset}
          style="transition: stroke-dashoffset 300ms ease"
        />
      </g>
    </svg>

    <div class="absolute inset-0 flex items-center justify-center">
      <div class="text-sm font-medium text-foreground tabular-nums">
        {session.completedCount} / {totalActive}
      </div>
    </div>
  </div>
</div>
