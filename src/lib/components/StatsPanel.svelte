<script lang="ts">
    import { Progress } from "$lib/components/ui/progress";
    import { Badge } from "$lib/components/ui/badge";
    import { Button } from "$lib/components/ui/button";
    import { Play, RefreshCw, FolderOpen } from "lucide-svelte";
    import type { ParsedMemory } from "$lib/parser";
    import { revealItemInDir } from "@tauri-apps/plugin-opener";

    let {
        memories,
        memoriesLength,
        completedCount,
        progressPercentage,
        isAllProcessed,
        selectedZip,
        selectedOutput,
        isProcessing,
        isPaused,
        onSelectOutput,
        onStartBackup,
        onTogglePause,
    } = $props<{
        memories: ParsedMemory[];
        memoriesLength: number;
        completedCount: number;
        progressPercentage: number;
        isAllProcessed: boolean;
        selectedZip: string | null;
        selectedOutput: string | null;
        isProcessing: boolean;
        isPaused: boolean;
        onSelectOutput: () => void;
        onStartBackup: () => void;
        onTogglePause: () => void;
    }>();

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
            <!-- Stats Text Breakdown -->
            <div class="flex flex-col gap-1">
                <div class="flex items-center gap-4 text-xs">
                    <div class="flex flex-col items-start leading-none">
                        <span class="text-muted-foreground">Total Memories</span
                        >
                        <span class="font-medium text-sm">{memoriesLength}</span
                        >
                    </div>
                    <div class="flex flex-col items-start leading-none">
                        <span class="text-muted-foreground">Completed</span>
                        <span class="font-medium text-green-500 text-sm"
                            >{completedCount}</span
                        >
                    </div>
                    <div class="flex flex-col items-start leading-none">
                        <span class="text-muted-foreground">Remaining</span>
                        <span class="font-medium text-sm"
                            >{memoriesLength - completedCount}</span
                        >
                    </div>
                </div>
                <div class="w-64 mt-1 flex items-center gap-2">
                    <Progress value={progressPercentage} class="h-1.5 flex-1" />
                </div>
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
                        class="border-primary/50 text-foreground"
                    >
                        Select Destination Folder
                    </Button>
                {:else}
                    <Button
                        variant="outline"
                        onclick={onSelectOutput}
                        class="text-xs truncate max-w-xs font-mono"
                        title={selectedOutput}
                    >
                        {selectedOutput}
                    </Button>
                    {#if isProcessing}
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
                            class="bg-primary text-primary-foreground hover:bg-primary/90 min-w-[140px]"
                        >
                            <Play class="mr-2 h-4 w-4" />
                            Start Backup
                        </Button>
                    {/if}
                {/if}
            {/if}
        </div>
    </div>
{/if}
