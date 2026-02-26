<script lang="ts">
    import { Progress } from "$lib/components/ui/progress";
    import { Badge } from "$lib/components/ui/badge";
    import { Button } from "$lib/components/ui/button";
    import { Play, RefreshCw, RotateCcw } from "lucide-svelte";
    import ThemeToggle from "./ThemeToggle.svelte";
    import SettingsPanel from "./SettingsPanel.svelte";
    import AboutPanel from "./AboutPanel.svelte";

    let {
        memoriesLength,
        completedCount,
        progressPercentage,
        isAllProcessed,
        selectedZip,
        selectedOutput,
        isProcessing,
        onSelectOutput,
        onStartBackup,
        onResetApp,
    } = $props<{
        memoriesLength: number;
        completedCount: number;
        progressPercentage: number;
        isAllProcessed: boolean;
        selectedZip: string | null;
        selectedOutput: string | null;
        isProcessing: boolean;
        onSelectOutput: () => void;
        onStartBackup: () => void;
        onResetApp: () => void;
    }>();
</script>

<header
    class="flex h-14 items-center justify-between border-b bg-card px-4 shrink-0"
>
    <div class="flex items-center gap-4">
        <div class="flex flex-col">
            <h1 class="text-sm font-semibold tracking-tight leading-none">
                YellowBox
            </h1>
            <p
                class="text-[10px] text-muted-foreground uppercase tracking-wider mt-1"
            >
                Memories backup tool
            </p>
        </div>

        {#if memoriesLength > 0}
            <div class="h-8 w-px bg-border mx-2"></div>
            <div class="flex items-center gap-4 text-xs">
                <div class="flex flex-col items-start leading-none">
                    <span class="text-muted-foreground">Total</span>
                    <span class="font-medium">{memoriesLength}</span>
                </div>
                <div class="flex flex-col items-start leading-none">
                    <span class="text-muted-foreground">Completed</span>
                    <span class="font-medium text-green-500"
                        >{completedCount}</span
                    >
                </div>
                <div class="w-48 ml-2">
                    <Progress value={progressPercentage} class="h-1.5" />
                </div>
            </div>
        {/if}
    </div>

    <div class="flex items-center gap-2">
        {#if selectedZip && memoriesLength > 0}
            {#if isAllProcessed}
                <Badge
                    class="bg-green-500 hover:bg-green-600 text-[11px] px-3 py-1 font-bold tracking-wider uppercase"
                >
                    Backup Complete
                </Badge>
            {:else if !selectedOutput}
                <Button
                    size="sm"
                    variant="outline"
                    onclick={onSelectOutput}
                    class="border-primary/50 text-foreground"
                >
                    Select Destination
                </Button>
            {:else}
                <Button
                    size="sm"
                    variant="outline"
                    onclick={onSelectOutput}
                    class="text-xs truncate max-w-sm font-mono mr-1"
                    title={selectedOutput}
                >
                    {selectedOutput}
                </Button>
                <Button
                    size="sm"
                    onclick={onStartBackup}
                    disabled={isProcessing}
                    class="bg-primary text-primary-foreground hover:bg-primary/90"
                >
                    {#if isProcessing}
                        <RefreshCw class="mr-2 h-3.5 w-3.5 animate-spin" />
                        Processing...
                    {:else}
                        <Play class="mr-2 h-3.5 w-3.5" />
                        Start Backup
                    {/if}
                </Button>
            {/if}
            <div class="w-px h-6 bg-border mx-1"></div>
        {/if}
        <ThemeToggle />
        <AboutPanel />
        <SettingsPanel {onResetApp} />
    </div>
</header>
