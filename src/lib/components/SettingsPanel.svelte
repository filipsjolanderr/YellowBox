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
    import { Settings } from "lucide-svelte";
    import { appConfig } from "$lib/config.svelte";

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
        appConfig.maxConcurrency = parsed != null && parsed >= 1 && parsed <= 32 ? parsed : null;
        appConfig.save();
        open = false;
    }

    function handleReset() {
        appConfig.resetPrefs();
        maxConcurrencyInput = "";
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
            <p>Settings & Configuration</p>
        </Tooltip.Content>
    </Tooltip.Root>
    <DialogContent>
        <DialogHeader>
            <DialogTitle>Configuration</DialogTitle>
            <DialogDescription>
                Adjust pipeline limits and file strategies. Changes apply on the
                next backup run.
            </DialogDescription>
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
                <input
                    id="maxConcurrency"
                    type="number"
                    min="1"
                    max="32"
                    placeholder="Auto (CPU cores)"
                    class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                    bind:value={maxConcurrencyInput}
                />
                <p class="text-[11px] text-muted-foreground">
                    Limit parallel processing. Leave empty for auto (CPU cores).
                    Lower values help with rate limits or disk I/O.
                </p>
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
