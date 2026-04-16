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

<div class="flex flex-col items-center justify-center p-4">
  <div class="relative h-[160px] w-[160px] group transition-transform duration-500 hover:scale-[1.02]">
    <!-- Ambient Glow behind the ring -->
    <div 
      class="absolute inset-4 bg-primary/20 blur-[32px] rounded-full animate-pulse transition-opacity duration-1000"
      style="opacity: {ratio > 0 ? 1 : 0}"
    ></div>

    <svg
      viewBox={`0 0 ${SIZE} ${SIZE}`}
      class="h-full w-full relative z-10 drop-shadow-[0_0_12px_rgba(var(--color-primary-rgb),0.2)]"
      aria-label="Overall progress"
      role="img"
    >
      <defs>
        <linearGradient id="progressGradient" x1="0%" y1="0%" x2="100%" y2="0%">
          <stop offset="0%" stop-color="var(--color-primary)" />
          <stop offset="100%" stop-color="oklch(0.92 0.22 93)" />
        </linearGradient>
      </defs>

      <circle
        cx={SIZE / 2}
        cy={SIZE / 2}
        r={R}
        fill="none"
        stroke="var(--color-secondary)"
        stroke-width={STROKE}
        class="opacity-40"
      />

      <g transform={`rotate(-90 ${SIZE / 2} ${SIZE / 2})`}>
        <circle
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={R}
          fill="none"
          stroke="url(#progressGradient)"
          stroke-width={STROKE}
          stroke-linecap="round"
          stroke-dasharray={C}
          stroke-dashoffset={dashOffset}
          style="transition: stroke-dashoffset 800ms cubic-bezier(0.34, 1.56, 0.64, 1)"
        />
      </g>
    </svg>

    <div class="absolute inset-0 flex flex-col items-center justify-center z-20">
      <div class="text-[10px] font-black uppercase tracking-[0.2em] text-muted-foreground/60 mb-1">
        Progress
      </div>
      <div class="text-2xl font-black text-foreground tabular-nums tracking-tighter drop-shadow-sm flex items-baseline">
        {session.completedCount}
        <span class="text-xs text-muted-foreground/50 mx-1.5 font-bold">/</span>
        <span class="text-lg text-muted-foreground/80">{totalActive}</span>
      </div>
    </div>
  </div>
</div>
