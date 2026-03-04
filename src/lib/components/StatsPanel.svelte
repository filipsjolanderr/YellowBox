<script lang="ts">
    import { Progress } from "$lib/components/ui/progress";
    import { Badge } from "$lib/components/ui/badge";
    import { Button } from "$lib/components/ui/button";
    import { Play, RefreshCw, FolderOpen, ChartNetwork } from "lucide-svelte";
    import type { ParsedMemory } from "$lib/parser";
    import { revealItemInDir } from "@tauri-apps/plugin-opener";
    import {
        Tooltip,
        TooltipContent,
        TooltipTrigger,
        TooltipProvider,
    } from "$lib/components/ui/tooltip";
    import StatusTracker from "./StatusTracker.svelte";
    import type { Session } from "$lib/session.svelte";

    let { session, onSelectOutput, onStartBackup, onTogglePause } = $props<{
        session: Session;
        onSelectOutput: () => void;
        onStartBackup: () => void;
        onTogglePause: () => void;
    }>();

    // Derived aliases for convenience if needed, or just use session.X
    let memoriesLength = $derived(session.totalCount);
    let isReadyForBackup = $derived(session.memories.length > 0);
    let completedCount = $derived(session.completedCount);
    let progressPercentage = $derived(session.progressPercentage);
    let isAllProcessed = $derived(session.isAllProcessed);
    let selectedZip = $derived(session.selectedZip);
    let selectedOutput = $derived(session.selectedOutput);
    let isProcessing = $derived(session.isProcessing);
    let isPaused = $derived(session.isPaused);

    async function handleOpenOutputFolder() {
        if (!selectedOutput) return;
        try {
            await revealItemInDir(selectedOutput);
        } catch (err) {
            console.error("Failed to open folder:", err);
        }
    }
</script>

{#if memoriesLength > 0}
    <div
        class="flex items-center justify-between border-b bg-card px-6 py-3 shrink-0"
    >
        <!-- Left: Stats & Chart -->
        <div class="flex items-center gap-6 h-full">
            <StatusTracker {session} />

            <div class="flex flex-col gap-2 min-w-[300px]">
                <div class="flex items-center justify-between text-xs">
                    <div class="flex items-center gap-2">
                        {#if isProcessing}
                            <RefreshCw
                                class="h-3 w-3 animate-spin text-primary"
                            />
                            <span class="font-medium text-foreground">
                                {isPaused ? "Paused" : "Processing Memories..."}
                            </span>
                        {:else if isAllProcessed}
                            <div
                                class="h-2 w-2 rounded-full bg-green-500"
                            ></div>
                            <span class="font-medium text-green-500"
                                >Backup complete</span
                            >
                        {:else}
                            <span class="text-muted-foreground"
                                >Ready to backup</span
                            >
                        {/if}
                    </div>
                    <span class="text-muted-foreground tabular-nums">
                        {Math.round(progressPercentage)}%
                    </span>
                </div>
                <Progress value={progressPercentage} class="h-1.5" />
            </div>
        </div>

        <!-- Right: Actions -->
        <div class="flex items-center gap-3">
            {#if selectedZip && isAllProcessed}
                <Button
                    onclick={handleOpenOutputFolder}
                    class="bg-green-500 hover:bg-green-600 text-white min-w-[140px]"
                >
                    <FolderOpen class="mr-2 h-4 w-4" />
                    Open Folder
                </Button>
            {:else if selectedZip}
                {#if !selectedOutput}
                    <Button
                        variant="outline"
                        onclick={onSelectOutput}
                        class="pulse-glow border-2 text-primary font-semibold shadow-md ring-2 ring-primary/20 hover:ring-primary/40 hover:bg-primary/5 min-w-[200px]"
                    >
                        <FolderOpen class="mr-2 h-4 w-4" />
                        Select destination folder
                    </Button>
                {:else}
                    <TooltipProvider delayDuration={150}>
                        <Tooltip>
                            <TooltipTrigger>
                                {#snippet child({ props })}
                                    <Button
                                        {...props}
                                        variant="outline"
                                        onclick={onSelectOutput}
                                        class="text-xs truncate max-w-xs font-mono bg-primary/5 hover:bg-primary/10"
                                    >
                                        <FolderOpen
                                            class="mr-1.5 h-3.5 w-3.5 shrink-0"
                                        />
                                        {selectedOutput}
                                    </Button>
                                {/snippet}
                            </TooltipTrigger>
                            <TooltipContent>
                                <span class=" text-xs"
                                    >Change destination folder</span
                                >
                            </TooltipContent>
                        </Tooltip>
                    </TooltipProvider>
                    {#if !isReadyForBackup}
                        <Button
                            disabled
                            class="min-w-[140px] opacity-70"
                            title="Waiting for setup to finish..."
                        >
                            <RefreshCw class="mr-2 h-4 w-4 animate-spin" />
                            Preparing...
                        </Button>
                    {:else if isProcessing}
                        <Button
                            onclick={onTogglePause}
                            variant={isPaused ? "default" : "secondary"}
                            class="font-medium min-w-[140px] transition-colors"
                        >
                            {#if isPaused}
                                <Play class="mr-2 h-4 w-4" />
                                Continue
                            {:else}
                                <!-- Use a standard pause icon logic, lucide-svelte has 'Pause' -->
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    width="24"
                                    height="24"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    class="mr-2 h-4 w-4 lucide lucide-pause"
                                    ><rect
                                        width="4"
                                        height="16"
                                        x="6"
                                        y="4"
                                    /><rect
                                        width="4"
                                        height="16"
                                        x="14"
                                        y="4"
                                    /></svg
                                >
                                Pause
                            {/if}
                        </Button>
                    {:else}
                        <Button
                            onclick={onStartBackup}
                            class="pulse-glow bg-primary text-primary-foreground hover:bg-primary/90 min-w-[140px] h-9 font-semibold shadow-md ring-2 ring-primary/30 hover:ring-primary/50"
                        >
                            <Play class="mr-2 h-4 w-4" />
                            Start backup
                        </Button>
                    {/if}
                {/if}
            {/if}
        </div>
    </div>
{/if}
