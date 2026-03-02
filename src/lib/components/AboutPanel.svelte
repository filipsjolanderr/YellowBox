<script lang="ts">
    import {
        Dialog,
        DialogContent,
        DialogDescription,
        DialogHeader,
        DialogTitle,
        DialogTrigger,
    } from "$lib/components/ui/dialog";
    import { Button } from "$lib/components/ui/button";
    import { Info } from "lucide-svelte";
    import * as Tooltip from "$lib/components/ui/tooltip";
</script>

<Dialog>
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
                                class="h-8 w-8 text-muted-foreground hover:text-foreground"
                            >
                                <Info class="h-4 w-4" />
                                <span class="sr-only">About</span>
                            </Button>
                        {/snippet}
                    </DialogTrigger>
                </div>
            {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content sideOffset={4}>
            <p>About YellowBox</p>
        </Tooltip.Content>
    </Tooltip.Root>
    <DialogContent class="sm:max-w-[425px]">
        <DialogHeader>
            <DialogTitle>About YellowBox</DialogTitle>
            <DialogDescription>v0.1.0</DialogDescription>
        </DialogHeader>
        <div class="py-4">
            <p class="text-sm text-foreground mb-4 font-medium">
                YellowBox is a desktop utility designed to securely parse,
                extract, and apply rich metadata to your exported Snapchat
                Memories data.
            </p>
            <p class="text-xs text-muted-foreground leading-relaxed">
                Built with Rust (Tauri) and Svelte 5.
                <br />
                Open Source & Privacy First. Data is processed entirely locally on
                your machine.
            </p>
        </div>
    </DialogContent>
</Dialog>
