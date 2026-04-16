<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Copy, Minus, Square, X } from "lucide-svelte";
  import ThemeToggle from "./ThemeToggle.svelte";
  import SettingsPanel from "./SettingsPanel.svelte";
  import AboutPanel from "./AboutPanel.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { onMount } from "svelte";

  const appWindow = getCurrentWindow();

  let isMaximized = $state(false);

  async function refreshIsMaximized() {
    try {
      isMaximized = await appWindow.isMaximized();
    } catch {
      // ignore (window might be closing)
    }
  }

  function minimize() {
    appWindow.minimize();
  }

  async function toggleMaximize() {
    await refreshIsMaximized();
    if (isMaximized) {
      appWindow.unmaximize();
    } else {
      appWindow.maximize();
    }
    // reflect new state ASAP; resize events may lag slightly on some platforms
    await refreshIsMaximized();
  }

  function close() {
    appWindow.close();
  }

  function startDrag(e: PointerEvent) {
    if (
      e.target instanceof Element &&
      e.target.closest('button, [role="button"], a, input, [role="tablist"]')
    ) {
      return;
    }
    appWindow.startDragging();
  }

  const windowControlBase =
    "h-8 w-8 rounded-md text-muted-foreground hover:text-foreground focus-visible:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-offset-0";

  onMount(() => {
    refreshIsMaximized();
    // Keep icon in sync even if user double-clicks title bar or uses OS shortcuts.
    const unsubs: Array<() => void> = [];

    appWindow
      .onResized(() => refreshIsMaximized())
      .then((unsub) => unsubs.push(unsub));
    appWindow
      .onFocusChanged(() => refreshIsMaximized())
      .then((unsub) => unsubs.push(unsub));

    return () => {
      for (const u of unsubs) u();
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<header
  data-tauri-drag-region
  onpointerdown={startDrag}
  class="flex h-14 items-center justify-between border-b border-border/40 backdrop-blur-xl bg-card/60 px-4 shrink-0 shadow-sm z-50 select-none sticky top-0"
>
  <div class="absolute inset-x-0 -bottom-px h-px bg-gradient-to-r from-transparent via-primary/20 to-transparent"></div>
  <!-- Left Section: Logo -->
  <div class="flex items-center flex-1 h-full" data-tauri-drag-region>
    <img
      src="/yellowbox.svg"
      alt="YellowBox Logo"
      class="h-6 w-6 select-none"
      draggable="false"
    />
  </div>

  <!-- Center Section: App Title -->
  <div
    class="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 pointer-events-none flex items-center justify-center"
    data-tauri-drag-region
  >
    <h1
      class="text-[13px] font-semibold leading-none pointer-events-auto cursor-default text-foreground/80 drop-shadow-sm"
    >
      YellowBox
    </h1>
  </div>

  <div class="flex items-center gap-2 flex-1 justify-end h-full">
    <ThemeToggle />
    <AboutPanel />
    <SettingsPanel />

    <div class="w-px h-6 bg-border mx-1"></div>

    <!-- Window Controls with Tooltips -->
    <div class="flex items-center gap-0.5 ml-1" data-tauri-drag-region="false">
      <Tooltip.Root>
        <Tooltip.Trigger>
          <Button
            variant="ghost"
            size="icon"
            class={`${windowControlBase} hover:bg-muted active:bg-muted/70`}
            onclick={minimize}
          >
            <Minus class="h-4 w-4" />
          </Button>
        </Tooltip.Trigger>
        <Tooltip.Content sideOffset={4}>
          <p>Minimize</p>
        </Tooltip.Content>
      </Tooltip.Root>

      <Tooltip.Root>
        <Tooltip.Trigger>
          <Button
            variant="ghost"
            size="icon"
            class={`${windowControlBase} hover:bg-muted active:bg-muted/70`}
            onclick={toggleMaximize}
          >
            {#if isMaximized}
              <Copy class="h-4 w-4" />
            {:else}
              <Square class="h-3 w-3" />
            {/if}
          </Button>
        </Tooltip.Trigger>
        <Tooltip.Content sideOffset={4}>
          <p>{isMaximized ? "Restore" : "Maximize"}</p>
        </Tooltip.Content>
      </Tooltip.Root>

      <Tooltip.Root>
        <Tooltip.Trigger>
          <Button
            variant="ghost"
            size="icon"
            class={`${windowControlBase} hover:bg-destructive hover:text-destructive-foreground active:scale-95 transition-all duration-200`}
            onclick={close}
          >
            <X class="h-4.5 w-4.5" />
          </Button>
        </Tooltip.Trigger>
        <Tooltip.Content sideOffset={4}>
          <p>Close</p>
        </Tooltip.Content>
      </Tooltip.Root>
    </div>
  </div>
</header>
