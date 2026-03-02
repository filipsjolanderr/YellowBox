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
    import { ExternalLink, Info, Mail, Download, Clock } from "lucide-svelte";

    let { trigger } = $props<{ trigger?: any }>();
</script>

<Dialog>
    {#if trigger}
        {@render trigger()}
    {:else}
        <DialogTrigger>
            {#snippet child({ props })}
                <Button
                    {...props}
                    variant="ghost"
                    class="text-xs text-muted-foreground hover:text-primary hover:bg-primary/5 border border-transparent hover:border-primary/20 px-3 py-1.5 h-auto flex items-center gap-1.5 transition-all rounded-full group"
                >
                    <Info class="h-3.5 w-3.5" />
                    <span class="font-semibold">Request Your Data</span>
                </Button>
            {/snippet}
        </DialogTrigger>
    {/if}

    <DialogContent class="sm:max-w-[480px]">
        <DialogHeader>
            <DialogTitle>Request Your Snapchat Data</DialogTitle>
            <DialogDescription>
                YellowBox requires a Snapchat data export to recover your
                memories.
            </DialogDescription>
        </DialogHeader>

        <div class="grid gap-6">
            <div class="space-y-4">
                <div class="flex gap-4">
                    <div
                        class="h-8 w-8 shrink-0 rounded-full bg-primary/10 flex items-center justify-center text-primary font-bold text-sm"
                    >
                        1
                    </div>
                    <div class="space-y-1">
                        <p class="text-sm font-medium leading-none">
                            Go to Snapchat Accounts
                        </p>
                        <p class="text-xs text-muted-foreground">
                            Log in to <a
                                href="https://accounts.snapchat.com"
                                target="_blank"
                                rel="noopener noreferrer"
                                class="text-primary hover:underline inline-flex items-center gap-0.5"
                                >accounts.snapchat.com<ExternalLink
                                    class="h-3 w-3"
                                /></a
                            >
                        </p>
                    </div>
                </div>

                <div class="flex gap-4">
                    <div
                        class="h-8 w-8 shrink-0 rounded-full bg-muted flex items-center justify-center text-muted-foreground font-bold text-sm"
                    >
                        2
                    </div>
                    <div class="space-y-1">
                        <p class="text-sm font-medium leading-none">
                            Navigate to "My Data"
                        </p>
                        <p class="text-xs text-muted-foreground">
                            Click on the <strong>My Data</strong>.
                        </p>
                    </div>
                </div>

                <div class="flex gap-4">
                    <div
                        class="h-8 w-8 shrink-0 rounded-full bg-muted flex items-center justify-center text-muted-foreground font-bold text-sm"
                    >
                        3
                    </div>
                    <div class="space-y-1">
                        <p class="text-sm font-medium leading-none">
                            Export your Memories
                        </p>
                        <p class="text-xs text-muted-foreground">
                            Select <strong>Export your Memories</strong> by toggling the
                            switch and click
                            <strong>Request only Memories</strong>.
                        </p>
                    </div>
                </div>

                <div class="flex gap-4">
                    <div
                        class="h-8 w-8 shrink-0 rounded-full bg-muted flex items-center justify-center text-muted-foreground font-bold text-sm"
                    >
                        4
                    </div>
                    <div class="space-y-1">
                        <p class="text-sm font-medium leading-none">
                            Set range to All Time & Submit
                        </p>
                        <p class="text-xs text-muted-foreground">
                            Set Date range to <strong>All Time</strong> and
                            click <strong>Submit</strong>.
                        </p>
                    </div>
                </div>

                <div class="flex gap-4">
                    <div
                        class="h-8 w-8 shrink-0 rounded-full bg-orange-100 flex items-center justify-center text-orange-600"
                    >
                        <Clock class="h-4 w-4" />
                    </div>
                    <div class="space-y-1">
                        <p class="text-sm font-medium leading-none">
                            Wait for Email
                        </p>
                        <p class="text-xs text-muted-foreground">
                            Snapchat will process your request. This can take
                            anywhere from a few hours to a few days.
                        </p>
                    </div>
                </div>

                <div class="flex gap-4">
                    <div
                        class="h-8 w-8 shrink-0 rounded-full bg-green-100 flex items-center justify-center text-green-600"
                    >
                        <Download class="h-4 w-4" />
                    </div>
                    <div class="space-y-1">
                        <p class="text-sm font-medium leading-none">
                            Download Your Data
                        </p>
                        <p class="text-xs text-muted-foreground">
                            Once ready, you'll receive an email. Click the link
                            to download the ZIP file.
                        </p>
                    </div>
                </div>
            </div>

            <div
                class="rounded-lg bg-muted/50 p-3 flex items-start gap-3 border border-border"
            >
                <Info class="h-4 w-4 text-primary shrink-0 mt-0.5" />
                <p class="text-[10px] leading-relaxed text-muted-foreground">
                    After downloading the ZIP, simply drag it into YellowBox to
                    begin your backup.
                </p>
            </div>
        </div>
    </DialogContent>
</Dialog>
