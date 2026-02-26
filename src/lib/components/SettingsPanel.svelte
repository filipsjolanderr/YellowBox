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
    import { Slider } from "$lib/components/ui/slider";
    import { Button } from "$lib/components/ui/button";
    import { Settings, RotateCcw } from "lucide-svelte";
    import { appConfig } from "$lib/config.svelte";

    import * as Tooltip from "$lib/components/ui/tooltip";

    let open = $state(false);

    function handleSave() {
        appConfig.save();
        open = false;
    }

    function handleReset() {
        appConfig.resetPrefs();
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
            <div class="space-y-3">
                <div class="flex items-center justify-between">
                    <Label for="concurrency" class="text-sm font-medium"
                        >Concurrency Limit</Label
                    >
                    <span
                        class="text-xs text-muted-foreground w-6 text-right font-mono"
                        >{appConfig.concurrencyLimit}</span
                    >
                </div>
                <Slider
                    id="concurrency"
                    type="single"
                    max={32}
                    min={1}
                    step={1}
                    bind:value={appConfig.concurrencyLimit}
                />
                <p class="text-[11px] text-muted-foreground">
                    Maximum number of files processed in parallel. Reduce this
                    if encountering OS file limits or network errors.
                </p>
            </div>

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
