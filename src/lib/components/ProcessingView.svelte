<script lang="ts">
    import type { Session } from "$lib/session.svelte";
    import { Progress } from "$lib/components/ui/progress";
    import { Badge } from "$lib/components/ui/badge";
    import { Button } from "$lib/components/ui/button";
    import { Separator } from "$lib/components/ui/separator";
    import {
        Play,
        Pause,
        RefreshCw,
        FolderOpen,
        Plus,
        Archive,
        CheckCircle2,
        FileArchive,
        X,
        Upload,
        CheckCheck,
        AlertCircle,
        Clock,
        Download,
        FolderArchive,
        Layers,
        CircleCheck,
        Server,
        HardDrive,
        LayoutGrid
    } from "lucide-svelte";
    import { fade, slide } from "svelte/transition";
    import StatusTracker from "./StatusTracker.svelte";
    import { revealItemInDir } from "@tauri-apps/plugin-opener";
    import { getCurrentWindow } from "@tauri-apps/api/window";

    let {
        session,
        onSelectOutput,
        onStartBackup,
        onTogglePause,
        onAddZip,
        onDropZip,
        onRemoveZip,
    } = $props<{
        session: Session;
        onSelectOutput: () => void;
        onStartBackup: () => void;
        onTogglePause: () => void;
        onAddZip: () => void;
        onDropZip: (path: string) => void;
        onRemoveZip: (path: string) => void;
    }>();

    function getFileName(path: string): string {
        return path.split(/[\\\/]/).pop() ?? path;
    }

    function getFileDir(path: string): string {
        const parts = path.split(/[\\\/]/);
        parts.pop();
        return parts.join("/") || path;
    }

    async function handleOpenOutputFolder() {
        if (!session.selectedOutput) return;
        try {
            await revealItemInDir(session.selectedOutput);
        } catch {}
    }

    // Drag and drop
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

    let viewState = $derived(() => {
        if (session.isAllProcessed) return 'finished';
        if (session.isProcessing) return 'processing';
        return 'setup';
    });

    let zipStats = $derived(() => {
        return session.selectedZips.map((zip: string, idx: number) => ({
            path: zip,
            name: getFileName(zip),
            dir: getFileDir(zip),
            index: idx,
        }));
    });

    let isReadyForBackup = $derived(
        session.memories.length > 0 && 
        !session.isParsing && 
        !session.isInitializingDb
    );

    // Stats items - Consolidated to avoid duplication
    let statItems = $derived([
        {
            label: "Queued",
            value: session.pendingCount,
            color: "text-muted-foreground",
            bg: "bg-muted/30",
            icon: Clock,
        },
        {
            label: "Acquired",
            value: session.downloadedCount,
            color: "text-blue-500",
            bg: "bg-blue-500/10",
            icon: Download,
        },
        {
            label: "Unpacked",
            value: session.extractedCount,
            color: "text-orange-500",
            bg: "bg-orange-500/10",
            icon: FolderArchive,
        },
        {
            label: "Composited",
            value: session.combinedCount,
            color: "text-yellow-500",
            bg: "bg-yellow-500/10",
            icon: Layers,
        },
        {
            label: "Done",
            value: session.completedCount,
            color: "text-green-500",
            bg: "bg-green-500/10",
            icon: CircleCheck,
        },
        {
            label: "Error",
            value: session.failedCount,
            color: "text-destructive",
            bg: "bg-destructive/10",
            icon: AlertCircle,
        },
    ]);

    let parsingProgressVal = $derived(session.parsingProgress);
</script>

<div class="flex flex-col h-full w-full bg-background overflow-hidden">


    <main class="flex-1 overflow-hidden relative">
        {#if viewState() === 'setup'}
            <div class="h-full w-full flex flex-col overflow-y-auto p-12 gap-10" in:fade>
                <div class="max-w-4xl w-full mx-auto ml-0">
                        <h2 class="text-2xl font-bold tracking-tight">Prepare your backup</h2>
                        <p class="text-muted-foreground mt-2">Connect your source archives and choose where to save the results.</p>
                    </div>

                    <div class="flex flex-col gap-8">
                        <!-- Destination Card -->
                        <section class="flex flex-col gap-3">
                            <div class="flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-muted-foreground">
                                <HardDrive class="h-3.5 w-3.5" />
                                Target Destination
                            </div>
                            
                            {#if !session.selectedOutput}
                                <button
                                    onclick={onSelectOutput}
                                    class="group flex items-center justify-between gap-4 px-6 py-8 bg-card border border-dashed rounded-2xl hover:border-primary/50 hover:bg-primary/5 transition-all duration-300 text-left"
                                    in:fade
                                >
                                    <div class="flex items-center gap-4">
                                        <div class="h-12 w-12 rounded-full bg-muted flex items-center justify-center group-hover:bg-primary/10 transition-colors">
                                            <FolderOpen class="h-6 w-6 text-muted-foreground group-hover:text-primary" />
                                        </div>
                                        <div>
                                            <p class="text-base font-semibold text-foreground">Select destination folder</p>
                                            <p class="text-sm text-muted-foreground mt-0.5">Where should we store your memories?</p>
                                        </div>
                                    </div>
                                    <Plus class="h-5 w-5 text-muted-foreground group-hover:text-primary transition-colors shrink-0" />
                                </button>
                            {:else}
                                <div class="flex items-center justify-between gap-4 px-6 py-5 bg-card rounded-2xl border border-border/60 shadow-sm" in:slide>
                                    <div class="flex items-center gap-4 min-w-0">
                                        <div class="h-12 w-12 rounded-lg bg-primary/10 text-primary flex items-center justify-center shrink-0">
                                            <FolderOpen class="h-6 w-6" />
                                        </div>
                                        <div class="flex flex-col min-w-0">
                                            <span class="text-base font-bold text-foreground truncate">{getFileName(session.selectedOutput)}</span>
                                            <span class="text-xs text-muted-foreground truncate font-mono mt-1">{session.selectedOutput}</span>
                                        </div>
                                    </div>
                                    <Button variant="ghost" size="sm" onclick={onSelectOutput} class="text-xs font-medium hover:text-primary h-9 px-4 shrink-0">
                                        Change
                                    </Button>
                                </div>
                            {/if}
                        </section>

                        <section class="flex flex-col gap-3">
                            <div class="flex items-center justify-between text-xs font-bold uppercase tracking-wider text-muted-foreground">
                                <div class="flex items-center gap-2">
                                    <Server class="h-3.5 w-3.5" />
                                    Memories ZIPs
                                </div>
                                <Badge variant="outline" class="text-[10px] bg-background font-mono px-2">
                                    {session.selectedZips.length}
                                </Badge>
                            </div>

                            <div class="flex flex-col border rounded-2xl overflow-hidden bg-muted/5">
                                <div class="p-6 flex flex-col gap-4">
                                {#if session.selectedZips.length === 0}
                                    <button
                                        onclick={onAddZip}
                                        class="w-full py-12 flex flex-col items-center justify-center gap-4 border-2 border-dashed rounded-xl transition-all duration-300
                                            {isDragging ? 'border-primary bg-primary/5 text-primary' : 'border-border/60 hover:border-primary/40 hover:bg-card text-muted-foreground'}"
                                    >
                                        <Upload class="h-8 w-8" />
                                        <div class="text-center">
                                            <p class="text-sm font-bold text-foreground">Add Memories ZIPs</p>
                                            <p class="text-[11px] mt-1">Drop your Snapchat export files here</p>
                                        </div>
                                    </button>
                                {:else}
                                    <div class="grid grid-cols-1 gap-2.5">
                                        {#each zipStats() as zip (zip.path)}
                                            <div class="group relative flex items-center gap-3 p-3 bg-card border rounded-xl shadow-sm hover:shadow-md transition-all" in:slide>
                                                <div class="h-8 w-8 rounded-lg bg-orange-500/10 text-orange-600 flex items-center justify-center shrink-0">
                                                    <FileArchive class="h-4 w-4" />
                                                </div>
                                                <div class="flex flex-col min-w-0 pr-8">
                                                    <span class="text-xs font-bold truncate">{zip.name}</span>
                                                </div>
                                                <Button
                                                    variant="ghost" 
                                                    size="icon"
                                                    class="absolute top-1/2 -translate-y-1/2 right-2 h-7 w-7 rounded-md opacity-0 group-hover:opacity-100 hover:bg-destructive/10 hover:text-destructive transition-all"
                                                    onclick={() => onRemoveZip(zip.path)}
                                                >
                                                    <X class="h-3.5 w-3.5" />
                                                </Button>
                                            </div>
                                        {/each}
                                    </div>
                                    <Button variant="outline" onclick={onAddZip} class="w-full border-dashed py-4 h-auto text-xs font-bold uppercase tracking-tighter gap-2">
                                        <Plus class="h-4 w-4" />
                                        Add More
                                    </Button>
                                {/if}

                                {#if session.isParsing || session.isInitializingDb}
                                    <div class="mt-4 p-5 bg-primary/5 rounded-2xl border border-primary/20 flex flex-col gap-4 shadow-sm" in:fade>
                                        <div class="flex items-center justify-between">
                                            <div class="flex items-center gap-3 font-bold text-xs text-primary">
                                                <RefreshCw class="h-3.5 w-3.5 animate-spin" />
                                                <span class="uppercase tracking-widest">
                                                    {session.isParsing ? "Indexing Memories" : "Preparing Database"}
                                                </span>
                                            </div>
                                            <span class="text-[10px] font-mono font-bold text-primary/70">
                                                {Math.round($parsingProgressVal)}%
                                            </span>
                                        </div>
                                        <Progress value={$parsingProgressVal} class="h-2 rounded-full bg-primary/10" />
                                        <p class="text-[10px] text-muted-foreground leading-snug font-medium">
                                            {session.isParsing 
                                                ? "Analyzing file structure and building memory index. This may take a moment..." 
                                                : "Finalizing local database for high-speed processing..."}
                                        </p>
                                    </div>
                                {/if}
                                </div>
                            </div>
                        </section>
                </div>
            </div>

            <!-- Floating Start Button - Bottom Right -->
            <div class="fixed bottom-12 right-12 z-10" in:fade>
                <Button
                    onclick={onStartBackup}
                    size="lg"
                    disabled={!isReadyForBackup}
                    class="gap-3 font-bold shadow-2xl shadow-primary/30 h-16 px-8 text-lg rounded-2xl"
                >
                    {#if session.isParsing || session.isInitializingDb}
                        <RefreshCw class="h-5 w-5 animate-spin" />
                        Indexing...
                    {:else}
                        <Play class="h-5 w-5 fill-current" />
                        Start Backup
                    {/if}
                </Button>
            </div>

        {:else if viewState() === 'processing'}
            <div class="h-full w-full flex flex-col items-center justify-center p-12 gap-12" in:fade>
                <div class="flex flex-col items-center text-center gap-4">
                    <div class="relative">
                        <div class="absolute inset-0 bg-primary/20 blur-3xl rounded-full animate-pulse"></div>
                        <div class="relative scale-[1.5]">
                            <StatusTracker {session} />
                        </div>
                    </div>
                    <div class="mt-8 flex flex-col items-center gap-2">
                        <h2 class="text-3xl font-black tracking-tight">{session.progressPercentage.toFixed(1)}%</h2>
                        <p class="text-muted-foreground font-medium">Processing your memories...</p>
                    </div>
                </div>

                <div class="w-full max-w-md flex flex-col gap-6">
                    <div class="p-6 bg-card rounded-2xl border shadow-xl flex flex-col gap-4 ring-1 ring-primary/10">
                        <div class="flex items-center justify-between">
                            <span class="text-sm font-bold opacity-70 uppercase tracking-widest">Progress</span>
                            <span class="text-sm font-black font-mono">
                                {session.completedCount + session.failedCount} <span class="opacity-40">/ {session.totalCount}</span>
                            </span>
                        </div>
                        <Progress value={session.progressPercentage} class="h-3 rounded-full" />
                        <div class="flex items-center justify-center gap-6 pt-2">
                            <div class="flex items-center gap-2 text-[11px]">
                                <div class="h-2 w-2 rounded-full bg-green-500"></div>
                                <span class="font-bold">{session.completedCount.toLocaleString()}</span>
                                <span class="text-muted-foreground leading-none">Completed</span>
                            </div>
                            {#if session.failedCount > 0}
                                <div class="flex items-center gap-2 text-[11px]">
                                    <div class="h-2 w-2 rounded-full bg-destructive"></div>
                                    <span class="font-bold">{session.failedCount.toLocaleString()}</span>
                                    <span class="text-muted-foreground leading-none">Errors</span>
                                </div>
                            {/if}
                        </div>
                    </div>
                    
                    <div class="flex items-center justify-center gap-3">
                        <p class="text-center text-[10px] text-muted-foreground font-medium animate-pulse">
                            Please keep the application open during this process
                        </p>
                        
                        <Separator orientation="vertical" class="h-4" />

                        <Button
                            onclick={onTogglePause}
                            variant="ghost"
                            size="sm"
                            class="gap-2 text-[10px] font-bold uppercase tracking-wider h-8 hover:bg-primary/5"
                        >
                            {#if session.isPaused}
                                <Play class="h-3 w-3 fill-current" />
                                Resume
                            {:else}
                                <Pause class="h-3 w-3 fill-current" />
                                Pause
                            {/if}
                        </Button>
                    </div>
                </div>
            </div>

        {:else if viewState() === 'finished'}
            <div class="h-full w-full flex flex-col overflow-y-auto p-12 gap-12" in:fade>
                <div class="flex flex-col items-center text-center gap-6 py-12">
                    <div class="h-24 w-24 rounded-full bg-green-500/10 text-green-500 flex items-center justify-center ring-8 ring-green-500/5">
                        <CheckCheck class="h-12 w-12" />
                    </div>
                    <div class="max-w-md">
                        <h2 class="text-4xl font-black tracking-tight">Process Completed</h2>
                        <p class="text-muted-foreground mt-3 text-lg">Your Snapchat backup has been successfully reconstructed and saved.</p>
                    </div>
                    <Button size="lg" onclick={handleOpenOutputFolder} class="bg-green-600 hover:bg-green-700 text-white gap-3 px-8 h-14 text-base font-bold rounded-2xl shadow-xl shadow-green-600/20">
                        <FolderOpen class="h-5 w-5" />
                        Explore Memories
                    </Button>
                </div>

                <div class="flex flex-col gap-8 max-w-xl mx-auto w-full">
                    <div class="p-8 bg-primary/5 rounded-3xl border border-primary/10 flex flex-col justify-center gap-4 text-center items-center">
                        <h3 class="text-xl font-bold">What's Next?</h3>
                        <p class="text-sm text-muted-foreground leading-relaxed">You can now safely delete the original ZIPs if you wish. All your memories are safely organized in the destination folder.</p>
                        <div class="flex gap-4 mt-2">
                            <Button variant="outline" class="h-10 rounded-xl font-bold px-6" onclick={() => location.reload()}>
                                Start New Backup
                            </Button>
                        </div>
                    </div>
                </div>
            </div>
        {/if}
    </main>
</div>
