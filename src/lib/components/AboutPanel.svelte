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
    import { Info, ShieldCheck, Cpu, FolderKey } from "lucide-svelte";
    import * as Tooltip from "$lib/components/ui/tooltip";

    const version = import.meta.env.VITE_APP_VERSION ?? "0.0.0";
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
            <DialogDescription>v{version}</DialogDescription>
        </DialogHeader>
        <div class="py-4 space-y-5">
            <p class="text-sm text-foreground font-medium leading-relaxed">
                YellowBox securely parses and reconstructs your exported Snapchat Memories — entirely on-device.
            </p>

            <div class="grid gap-3">
                <div class="flex items-start gap-3">
                    <ShieldCheck class="h-4 w-4 text-primary mt-0.5" />
                    <div class="min-w-0">
                        <p class="text-xs font-semibold">Privacy-first</p>
                        <p class="text-[11px] text-muted-foreground leading-relaxed">
                            Your data stays on your machine. No uploads, no accounts.
                        </p>
                    </div>
                </div>
                <div class="flex items-start gap-3">
                    <Cpu class="h-4 w-4 text-primary mt-0.5" />
                    <div class="min-w-0">
                        <p class="text-xs font-semibold">Built for speed</p>
                        <p class="text-[11px] text-muted-foreground leading-relaxed">
                            Parallel processing with configurable limits.
                        </p>
                    </div>
                </div>
                <div class="flex items-start gap-3">
                    <FolderKey class="h-4 w-4 text-primary mt-0.5" />
                    <div class="min-w-0">
                        <p class="text-xs font-semibold">Local output</p>
                        <p class="text-[11px] text-muted-foreground leading-relaxed">
                            Results are saved to the folder you choose, ready to browse.
                        </p>
                    </div>
                </div>
            </div>

            <div class="rounded-xl border border-border/60 bg-muted/20 p-3">
                <p class="text-[11px] text-muted-foreground leading-relaxed">
                    Built with Rust (Tauri) and Svelte 5.
                </p>
            </div>

            <div class="text-[11px] text-muted-foreground">
                Made by <span class="font-medium text-foreground">Filip Sjölander</span> ·
                <a
                    class="text-primary underline underline-offset-4 hover:opacity-90"
                    href="https://sjolander.dev"
                    target="_blank"
                    rel="noreferrer"
                >
                    sjolander.dev
                </a>
            </div>
        </div>
    </DialogContent>
</Dialog>
