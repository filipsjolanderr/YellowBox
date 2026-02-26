<script lang="ts">
    import { Button } from "$lib/components/ui/button";
    import * as Tabs from "$lib/components/ui/tabs";
    import { Minus, Square, X, PlusCircle, XCircle } from "lucide-svelte";
    import ThemeToggle from "./ThemeToggle.svelte";
    import SettingsPanel from "./SettingsPanel.svelte";
    import AboutPanel from "./AboutPanel.svelte";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import * as Tooltip from "$lib/components/ui/tooltip";
    import type { Session } from "$lib/session.svelte";

    let { tabs, activeTabId, onTabChange, onNewTab, onCloseTab } = $props<{
        tabs: Session[];
        activeTabId: string;
        onTabChange: (id: string) => void;
        onNewTab: () => void;
        onCloseTab: (id: string) => void;
    }>();

    const appWindow = getCurrentWindow();

    function minimize() {
        appWindow.minimize();
    }

    async function toggleMaximize() {
        const isMaximized = await appWindow.isMaximized();
        if (isMaximized) {
            appWindow.unmaximize();
        } else {
            appWindow.maximize();
        }
    }

    function close() {
        appWindow.close();
    }

    function startDrag(e: PointerEvent) {
        if (
            e.target instanceof Element &&
            e.target.closest(
                'button, [role="button"], a, input, [role="tablist"]',
            )
        ) {
            return;
        }
        appWindow.startDragging();
    }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<header
    data-tauri-drag-region
    onpointerdown={startDrag}
    class="flex h-14 items-center justify-between border-b bg-card px-4 shrink-0 relative select-none"
>
    <!-- Left Section: Tabs -->
    <div class="flex items-end flex-1 h-full pt-2" data-tauri-drag-region>
        <div
            data-tauri-drag-region="false"
            class="flex items-center w-full h-full max-w-[600px] overflow-x-auto no-scrollbar"
        >
            <Tabs.Root
                value={activeTabId}
                onValueChange={(v) => onTabChange(v)}
                class="h-full flex items-end"
            >
                <Tabs.List
                    class="h-10 px-2 border-t border-x border-border/50 bg-muted/30"
                >
                    {#each tabs as tab}
                        <Tabs.Trigger
                            value={tab.id}
                            class="relative group flex items-center gap-1.5 px-3 data-[state=active]:bg-background data-[state=active]:shadow-sm cursor-pointer"
                        >
                            <span class="font-medium">{tab.name}</span>
                            {#if tab.totalCount > 0}
                                <span
                                    class="text-[10px] flex items-center justify-center  text-muted-foreground font-mono bg-muted px-1.5 py-0.5 rounded-sm"
                                >
                                    {tab.completedCount}/{tab.totalCount}
                                </span>
                            {/if}
                            <Tooltip.Root>
                                <Tooltip.Trigger>
                                    <button
                                        onclick={(e) => {
                                            e.stopPropagation();
                                            onCloseTab(tab.id);
                                        }}
                                        class="flex items-center justify-center text-muted-foreground hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity -mr-1 cursor-pointer"
                                    >
                                        <XCircle class="h-3.5 w-3.5" />
                                    </button>
                                </Tooltip.Trigger>
                                <Tooltip.Content sideOffset={4}>
                                    <p>Close Tab</p>
                                </Tooltip.Content>
                            </Tooltip.Root>
                        </Tabs.Trigger>
                    {/each}
                    {#if tabs.length < 3}
                        <button
                            onclick={onNewTab}
                            class="ml-2 px-2 py-1 text-muted-foreground hover:text-foreground flex items-center gap-1 text-sm bg-transparent hover:bg-muted/50 rounded-md transition-colors cursor-pointer"
                        >
                            <PlusCircle class="h-4 w-4" />
                            <span class="sr-only">New Backup</span>
                        </button>
                    {/if}
                </Tabs.List>
            </Tabs.Root>
        </div>
    </div>

    <!-- Center Section: App Title -->
    <div
        class="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 pointer-events-none flex items-center justify-center"
        data-tauri-drag-region
    >
        <h1
            class="text-sm font-semibold tracking-tight leading-none pointer-events-auto cursor-default"
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
        <div
            class="flex items-center gap-0.5 ml-1"
            data-tauri-drag-region="false"
        >
            <Tooltip.Root>
                <Tooltip.Trigger>
                    <Button
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8 text-muted-foreground hover:text-foreground hover:bg-muted"
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
                        class="h-8 w-8 text-muted-foreground hover:text-foreground hover:bg-muted"
                        onclick={toggleMaximize}
                    >
                        <Square class="h-3 w-3" />
                    </Button>
                </Tooltip.Trigger>
                <Tooltip.Content sideOffset={4}>
                    <p>Maximize</p>
                </Tooltip.Content>
            </Tooltip.Root>

            <Tooltip.Root>
                <Tooltip.Trigger>
                    <Button
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8 text-muted-foreground hover:bg-red-500 hover:text-white focus-visible:ring-0"
                        onclick={close}
                    >
                        <X class="h-4 w-4" />
                    </Button>
                </Tooltip.Trigger>
                <Tooltip.Content sideOffset={4}>
                    <p>Close</p>
                </Tooltip.Content>
            </Tooltip.Root>
        </div>
    </div>
</header>
