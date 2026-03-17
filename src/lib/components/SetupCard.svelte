<script lang="ts">
    import {
        Card,
        CardHeader,
        CardTitle,
        CardDescription,
        CardContent,
        CardFooter,
    } from "$lib/components/ui/card";
    import { Button } from "$lib/components/ui/button";
    import {
        FolderOpen,
        Upload,
        RefreshCw,
        X,
        FileJson,
        CheckCircle2,
    } from "lucide-svelte";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { Progress } from "$lib/components/ui/progress";
    import { slide, fade } from "svelte/transition";
    import { Badge } from "$lib/components/ui/badge";
    import { ScrollArea } from "$lib/components/ui/scroll-area";
    import DataRequestGuide from "./DataRequestGuide.svelte";

    let {
        selectedZips,
        isParsing,
        progressValue,
        onSelectZip,
        onDropZip,
        onRemoveZip,
    } = $props<{
        selectedZips: string[];
        isParsing: boolean;
        progressValue: number;
        onSelectZip: () => void;
        onDropZip: (path: string) => void;
        onRemoveZip: (path: string) => void;
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
                        for (const path of paths) {
                            if (path.toLowerCase().endsWith(".zip")) {
                                onDropZip(path);
                            }
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

    function getFileName(path: string) {
        return path.split(/[\\/]/).pop() || path;
    }
</script>

<div class="absolute inset-0 flex flex-col items-center justify-center p-4">
    <Card
        class="w-full max-w-md rounded-2xl shadow-2xl border-muted/50 bg-background/95 backdrop-blur-sm py-0 pt-6"
    >
        <CardHeader class="flex flex-col items-center text-center pt-8">
            <div
                class="h-14 w-14 rounded-2xl bg-primary/10 flex items-center justify-center mb-4 text-primary shadow-inner"
            >
                <Upload class="h-7 w-7" />
            </div>
            <CardTitle class="text-2xl font-bold tracking-tight">
                Import Snapchat Data
            </CardTitle>
            <CardDescription
                class="text-sm text-muted-foreground mt-2 max-w-[280px]"
            >
                Drop your Snapchat export ZIP files here to start the backup.
            </CardDescription>
        </CardHeader>

        <CardContent class="flex flex-col pt-0 w-full px-8">
            <!-- Dropzone -->
            <button
                onclick={onSelectZip}
                disabled={isParsing}
                class="w-full group relative overflow-hidden h-32 border-2 border-dashed {isDragging
                    ? 'border-primary bg-primary/5 ring-4 ring-primary/10'
                    : 'border-muted-foreground/20 hover:border-primary/40 hover:bg-muted/30'} rounded-2xl flex flex-col items-center justify-center transition-all duration-300 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
            >
                <div class="flex flex-col items-center gap-2">
                    {#if isParsing}
                        <RefreshCw class="h-6 w-6 animate-spin text-primary" />
                        <span class="text-sm font-semibold text-primary"
                            >Scanning Archive...</span
                        >
                    {:else}
                        <FolderOpen
                            class="h-6 w-6 text-muted-foreground group-hover:text-primary transition-colors"
                        />
                        <span class="text-sm font-semibold"
                            >Click or Drag & Drop</span
                        >
                        <span
                            class="text-[10px] text-muted-foreground uppercase tracking-widest font-bold"
                            >Snapchat .zip files</span
                        >
                    {/if}
                </div>
            </button>

            <!-- Zip List -->
            {#if selectedZips.length > 0}
                <div class="flex flex-col gap-3" transition:slide>
                    <div class="flex items-center justify-between px-1">
                        <span
                            class="text-xs font-bold uppercase tracking-wider text-muted-foreground"
                            >Selected Archives</span
                        >
                        <Badge
                            variant="secondary"
                            class="text-[10px] h-5 px-2 bg-primary/5 text-primary border-primary/10"
                        >
                            {selectedZips.length}
                            {selectedZips.length === 1 ? "File" : "Files"}
                        </Badge>
                    </div>

                    <ScrollArea
                        class="h-auto max-h-[160px] w-full rounded-xl border border-muted/50 bg-muted/20 p-2"
                    >
                        <div class="flex flex-col gap-2">
                            {#each selectedZips as zip (zip)}
                                <div
                                    class="flex items-center justify-between gap-3 p-2 bg-background rounded-lg border border-muted/50 group/item hover:border-primary/20 transition-all shadow-sm"
                                    transition:fade
                                >
                                    <div
                                        class="flex items-center gap-3 overflow-hidden"
                                    >
                                        <div
                                            class="h-8 w-8 rounded-lg bg-orange-500/10 text-orange-600 flex items-center justify-center shrink-0"
                                        >
                                            <FileJson class="h-4 w-4" />
                                        </div>
                                        <div
                                            class="flex flex-col overflow-hidden"
                                        >
                                            <span
                                                class="text-xs font-medium truncate pr-2"
                                            >
                                                {getFileName(zip)}
                                            </span>
                                            <div
                                                class="flex items-center gap-1.5 mt-0.5"
                                            >
                                                <Badge
                                                    variant="outline"
                                                    class="text-[9px] h-4 px-1 px-1.5 font-bold uppercase tracking-tighter bg-green-500/5 text-green-600 border-green-500/20"
                                                >
                                                    JSON Detected
                                                </Badge>
                                                <div
                                                    class="flex items-center gap-1 text-[9px] text-muted-foreground font-medium"
                                                >
                                                    <CheckCircle2
                                                        class="h-2.5 w-2.5 text-green-500"
                                                    />
                                                    Valid Structure
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                    <Button
                                        variant="ghost"
                                        size="icon"
                                        class="h-7 w-7 rounded-md hover:bg-destructive/10 hover:text-destructive shrink-0 transition-colors"
                                        onclick={() => onRemoveZip(zip)}
                                    >
                                        <X class="h-3.5 w-3.5" />
                                    </Button>
                                </div>
                            {/each}
                        </div>
                    </ScrollArea>
                </div>
            {/if}

            {#if isParsing}
                <div class="w-full space-y-3" transition:slide>
                    <div class="flex justify-between items-end">
                        <div class="space-y-1">
                            <span
                                class="text-xs font-bold uppercase tracking-wider text-primary"
                                >Indexing Media</span
                            >
                            <p class="text-[10px] text-muted-foreground">
                                Building cross-archive file index
                            </p>
                        </div>
                        <span class="text-xs font-mono font-bold text-primary"
                            >{Math.round(progressValue)}%</span
                        >
                    </div>
                    <Progress
                        value={progressValue}
                        class="h-1.5 w-full bg-primary/10"
                    />
                </div>
            {/if}

            <div class="flex justify-center pt-6">
                <DataRequestGuide />
            </div>
        </CardContent>
        <CardFooter
            class="pb-8 pt-2 flex justify-center border-t border-muted/50 mt-4 bg-muted/10 rounded-b-2xl"
        >
            <p
                class="text-[10px] text-muted-foreground font-medium flex items-center gap-1.5"
            >
                <CheckCircle2 class="h-4 w-4 text-primary" />
                Locally processed • No data leaves your computer
            </p>
        </CardFooter>
    </Card>
</div>
```
