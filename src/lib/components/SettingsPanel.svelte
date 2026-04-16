<script lang="ts">
    import {
        Dialog,
        DialogContent,
        DialogDescription,
        DialogFooter,
        DialogHeader,
        DialogTitle,
        DialogTrigger,
    } from "$lib/components/ui/dialog";
    import { Label } from "$lib/components/ui/label";
    import { Switch } from "$lib/components/ui/switch";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { Settings } from "lucide-svelte";
    import { appConfig } from "$lib/config.svelte";
    import { ask } from "@tauri-apps/plugin-dialog";
    import { tauriService } from "$lib/services/tauri";

    import * as Tooltip from "$lib/components/ui/tooltip";

    let open = $state(false);
    let maxConcurrencyInput = $state("");

    $effect(() => {
        if (open) {
            maxConcurrencyInput = appConfig.maxConcurrency != null ? String(appConfig.maxConcurrency) : "";
        }
    });

    function handleSave() {
        const parsed = maxConcurrencyInput.trim() ? parseInt(maxConcurrencyInput, 10) : null;
        appConfig.maxConcurrency = parsed != null && parsed >= 1 && parsed <= 128 ? parsed : null;
        appConfig.save();
        open = false;
    }

    function handleReset() {
        appConfig.resetPrefs();
        maxConcurrencyInput = "";
    }

    async function handleHardReset() {
        const confirmed = await ask(
            "This will permanently delete ALL local session databases and clear your current work. You will need to re-index your ZIPs. Continue?",
            { title: "Hard Reset", kind: "warning" }
        );
        
        if (confirmed) {
            try {
                // Also clear the saved paths in localStorage so they don't auto-load
                appConfig.lastZips = [];
                appConfig.lastOutput = null;
                appConfig.lastSessionId = null;
                appConfig.save();

                await tauriService.clearAllData();
                location.reload();
            } catch (err) {
                console.error("Hard reset failed:", err);
            }
        }
    }
</script>

<Dialog bind:open>
    <Tooltip.Root>
        <Tooltip.Trigger>
            {#snippet child({ props: tooltipProps })}
                <div {...tooltipProps} class="inline-block">
                    <DialogTrigger>
                        {#snippet child({ props: dialogProps })}
                            <Button
                                {...dialogProps}
                                variant="ghost"
                                size="icon"
                                class="h-8 w-8 text-muted-foreground hover:text-foreground relative group"
                            >
                                <Settings class="h-4 w-4" />
                                <span class="sr-only">Settings</span>
                            </Button>
                        {/snippet}
                    </DialogTrigger>
                </div>
            {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content sideOffset={4}>
            <p>Settings</p>
        </Tooltip.Content>
    </Tooltip.Root>
    <DialogContent>
        <DialogHeader>
            <DialogTitle>Settings</DialogTitle>
        </DialogHeader>

        <div class="grid gap-6 py-6">
            <div class="flex items-center justify-between space-x-4">
                <div class="flex flex-col space-y-1">
                    <Label for="overwrite" class="text-sm font-medium"
                        >Overwrite Existing Files</Label
                    >
                    <p class="text-[11px] text-muted-foreground">
                        By default, YellowBox skips downloaded files to resume
                        interruptions safely. Turn this on to force
                        re-processing.
                    </p>
                </div>
                <Switch
                    id="overwrite"
                    bind:checked={appConfig.overwriteExisting}
                />
            </div>
            <div class="flex flex-col space-y-2">
                <Label for="maxConcurrency" class="text-sm font-medium"
                    >Max Concurrent Items</Label
                >
                <Input
                    id="maxConcurrency"
                    type="number"
                    min="1"
                    max="32"
                    placeholder="Auto (CPU cores)"
                    bind:value={maxConcurrencyInput}
                />
                <p class="text-[11px] text-muted-foreground">
                    Limit parallel processing. Leave empty for auto (CPU cores).
                    Lower values help with rate limits or disk I/O.
                </p>
            </div>

            <div class="flex flex-col gap-2 pt-4 border-t border-border/50">
                <Label class="text-sm font-bold text-destructive"
                    >Danger Zone</Label
                >
                <div class="flex items-center justify-between gap-4">
                    <div class="flex flex-col space-y-1">
                        <p class="text-[11px] text-muted-foreground">
                            Clears all local databases and indexed metadata.
                            Use this if you need to force a full re-import.
                        </p>
                    </div>
                    <Button 
                        variant="ghost" 
                        size="sm" 
                        onclick={handleHardReset}
                        class="text-destructive hover:bg-destructive/10 hover:text-destructive whitespace-nowrap shrink-0"
                    >
                        Reset All Data
                    </Button>
                </div>
            </div>
        </div>
        <DialogFooter>
            <div class="flex space-x-2 w-full mt-2 sm:mt-0 justify-end">
                <Button variant="outline" onclick={handleReset}
                    >Reset Defaults</Button
                >
                <Button onclick={handleSave}>Save Settings</Button>
            </div>
        </DialogFooter>
    </DialogContent>
</Dialog>
