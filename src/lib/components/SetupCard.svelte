<script lang="ts">
    import {
        Card,
        CardHeader,
        CardTitle,
        CardDescription,
        CardContent,
    } from "$lib/components/ui/card";
    import { Label } from "$lib/components/ui/label";
    import { Input } from "$lib/components/ui/input";
    import { Button } from "$lib/components/ui/button";
    import { FolderOpen, Upload, RefreshCw } from "lucide-svelte";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { Progress } from "$lib/components/ui/progress";
    import { slide } from "svelte/transition";

    let { selectedZip, isParsing, progressValue, onSelectZip, onDropZip } =
        $props<{
            selectedZip: string | null;
            isParsing: boolean;
            progressValue: number;
            onSelectZip: () => void;
            onDropZip: (path: string) => void;
        }>();

    let isDragging = $state(false);

    $effect(() => {
        let unlisten: (() => void) | undefined;
        let isMounted = true;

        getCurrentWindow()
            .onDragDropEvent((event) => {
                if (!isMounted) return;
                if (
                    event.payload.type === "enter" ||
                    event.payload.type === "over"
                ) {
                    isDragging = true;
                } else if (event.payload.type === "leave") {
                    isDragging = false;
                } else if (event.payload.type === "drop") {
                    isDragging = false;
                    const paths = event.payload.paths;
                    if (paths && paths.length > 0) {
                        const first = paths[0];
                        if (first.toLowerCase().endsWith(".zip")) {
                            onDropZip(first);
                        }
                    }
                }
            })
            .then((fn) => {
                unlisten = fn;
            })
            .catch((err) => console.error(err));

        return () => {
            isMounted = false;
            if (unlisten) unlisten();
        };
    });
</script>

<div class="absolute inset-0 flex flex-col items-center justify-center">
    <Card class="w-full max-w-sm rounded-xl">
        <CardHeader class="flex flex-col items-center text-center pb-4 pt-6">
            <CardTitle class="text-lg font-semibold tracking-tight">
                Start Backup
            </CardTitle>
            <CardDescription class="text-xs text-muted-foreground mt-1">
                Select your Snapchat data to start the backup process.
            </CardDescription>
        </CardHeader>

        <CardContent
            class="flex flex-col gap-5 pt-0 pb-6 w-full transition-all duration-300"
        >
            <button
                onclick={onSelectZip}
                disabled={isParsing}
                class="w-full h-48 border-2 border-dashed {isDragging
                    ? 'border-primary bg-primary/10'
                    : 'border-muted-foreground/25 hover:border-primary/50 hover:bg-primary/5'} rounded-xl flex flex-col items-center justify-center transition-all cursor-pointer group disabled:opacity-50 disabled:cursor-not-allowed"
            >
                <div
                    class="h-12 w-12 rounded-full {isDragging
                        ? 'bg-primary/20 text-primary'
                        : 'bg-muted group-hover:bg-background text-muted-foreground group-hover:text-primary'} flex items-center justify-center mb-3 transition-colors"
                >
                    {#if isDragging}
                        <Upload
                            class="h-6 w-6 transition-colors animate-bounce"
                        />
                    {:else if isParsing}
                        <RefreshCw
                            class="h-6 w-6 transition-colors animate-spin text-primary"
                        />
                    {:else}
                        <FolderOpen class="h-6 w-6 transition-colors" />
                    {/if}
                </div>
                <span
                    class="text-sm font-semibold {isDragging || isParsing
                        ? 'text-primary'
                        : 'text-foreground'}"
                >
                    {#if isParsing}
                        Parsing Snapchat Data...
                    {:else if isDragging}
                        Drop zip file here
                    {:else}
                        Click to Browse
                    {/if}
                </span>
                {#if !isParsing}
                    <span
                        class="text-xs text-muted-foreground mt-1 transition-opacity"
                        >or drag & drop your generic ZIP here</span
                    >
                {/if}
            </button>

            {#if isParsing}
                <div class="w-full mt-2" transition:slide={{ duration: 300 }}>
                    <div
                        class="flex justify-between text-xs mb-1.5 font-medium text-muted-foreground"
                    >
                        <span>Reading Archive</span>
                        <span>{Math.round(progressValue)}%</span>
                    </div>
                    <Progress value={progressValue} class="h-2 w-full" />
                </div>
            {/if}
        </CardContent>
    </Card>
</div>
```
