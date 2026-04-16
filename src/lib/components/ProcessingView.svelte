<script lang="ts">
  import type { Session } from "$lib/session.svelte";
  import type { ParsedMemory } from "$lib/parser";
  import { Badge } from "$lib/components/ui/badge";
  import {
    Card,
    CardContent,
    CardDescription,
    CardFooter,
    CardHeader,
    CardTitle,
  } from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
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
    ImageIcon,
  } from "lucide-svelte";
  import { fade, slide } from "svelte/transition";
  import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogDescription,
  } from "$lib/components/ui/dialog";
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
    onCancelAndReset,
    onRetryMemory,
  } = $props<{
    session: Session;
    onSelectOutput: () => void;
    onStartBackup: () => void;
    onTogglePause: () => void;
    onAddZip: () => void;
    onDropZip: (path: string) => void;
    onRemoveZip: (path: string) => void;
    onCancelAndReset: () => void;
    onRetryMemory: (itemId: string) => void;
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
        if (event.payload.type === "enter" || event.payload.type === "over") {
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

  let viewState = $derived.by(() => {
    if (session.isAllProcessed) return "finished";
    if (session.isProcessing) return "processing";
    return "setup";
  });

  let zipStats = $derived.by(() => {
    return session.selectedZips.map((zip: string, idx: number) => ({
      path: zip,
      name: getFileName(zip),
      dir: getFileDir(zip),
      index: idx,
      count: session.memoryIdsByZip.get(zip)?.length ?? 0,
    }));
  });

  type ZipSequenceIssue = {
    base: string;
    missingParts: number[];
  };

  function analyzeMyDataZipSequence(paths: string[]): ZipSequenceIssue[] {
    // Only considers ZIPs the user explicitly added (privacy).
    // Pattern: mydata~<epoch>.zip or mydata~<epoch>-<part>.zip
    const re = /(?:^|[\\/])mydata~(\d+)(?:-(\d+))?\.zip$/i;
    const byBase = new Map<string, Set<number>>();

    for (const p of paths) {
      const m = p.match(re);
      if (!m) continue;
      const base = m[1];
      const part = m[2] ? Number(m[2]) : 1;
      if (!Number.isFinite(part) || part <= 0) continue;
      if (!byBase.has(base)) byBase.set(base, new Set());
      byBase.get(base)!.add(part);
    }

    const issues: ZipSequenceIssue[] = [];
    for (const [base, partsSet] of byBase.entries()) {
      const parts = Array.from(partsSet).sort((a, b) => a - b);
      if (parts.length <= 1) continue;
      const min = parts[0];
      const max = parts[parts.length - 1];
      const missing: number[] = [];
      for (let i = min; i <= max; i++) {
        if (!partsSet.has(i)) missing.push(i);
      }
      if (missing.length > 0) {
        issues.push({ base, missingParts: missing });
      }
    }

    return issues;
  }

  let zipSequenceIssues = $derived(
    analyzeMyDataZipSequence(session.selectedZips),
  );
  let hasZipSequenceGaps = $derived(zipSequenceIssues.length > 0);

  let isReadyForBackup = $derived(
    session.memories.length > 0 &&
      !session.isParsing &&
      !session.isInitializingDb,
  );

  let allZipsValid = $derived(
    session.selectedZips.length > 0 &&
      session.selectedZips.every(
        (z: string) => session.zipValidity.get(z) === "valid",
      ),
  );

  let canStartBackup = $derived(
    isReadyForBackup && allZipsValid && !hasZipSequenceGaps,
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

  let errorsOpen = $state(false);
  let failedMemories = $derived.by(() =>
    session.memories.filter((m: ParsedMemory) => m.state === "Failed"),
  );

  let previewItems = $derived.by(() =>
    session.memories.length > 0 ? session.memories : session.parsedItems,
  );

  function formatPreviewDate(value: string | null | undefined): string {
    if (!value) return "—";
    const d = new Date(value);
    if (Number.isNaN(d.getTime())) return "—";
    return d.toLocaleDateString();
  }

  let previewImageCount = $derived.by(
    () =>
      previewItems.filter((m: ParsedMemory) => m.mediaType === "Image").length,
  );
  let previewVideoCount = $derived.by(
    () =>
      previewItems.filter((m: ParsedMemory) => m.mediaType === "Video").length,
  );

  let previewDateRange = $derived.by(() => {
    const times = previewItems
      .map((m: ParsedMemory) => new Date(m.originalDate).getTime())
      .filter((t: number) => Number.isFinite(t));
    if (times.length === 0)
      return { start: null as string | null, end: null as string | null };
    const start = new Date(Math.min(...times)).toISOString();
    const end = new Date(Math.max(...times)).toISOString();
    return { start, end };
  });

  let zipPreviewCounts = $derived.by(() =>
    session.selectedZips.map((zip: string) => ({
      zip,
      count: session.memoryIdsByZip.get(zip)?.length ?? 0,
    })),
  );
</script>

<div
  class="flex flex-col h-full w-full bg-background overflow-hidden selection:bg-primary/30"
>
  <main class="flex flex-col flex-1 overflow-hidden relative">
    {#if viewState === "setup"}
      <!-- Scrollable Content Area -->
      <div class="flex-1 overflow-y-auto p-12 custom-scrollbar">
        <div class="max-w-5xl w-full mx-auto flex flex-col gap-12 pb-10">
          <div>
            <h2 class="text-3xl font-bold tracking-tight leading-tight">
              Prepare your backup
            </h2>
            <p class="text-muted-foreground mt-2 leading-relaxed text-base">
              Add your memories ZIPs and choose where to save the backup.
            </p>
          </div>

          <div class="grid grid-cols-2 gap-12 items-start">
            <!-- Memories ZIPs (Left Column) -->
            <div class="flex flex-col gap-5">
              <div class="flex items-start justify-between">
                <div class="flex flex-col gap-1.5">
                  <h3
                    class="flex items-center gap-2 text-lg font-bold tracking-tight"
                  >
                    <ImageIcon class="h-4.5 w-4.5 text-muted-foreground" />
                    Memories ZIPs
                  </h3>
                  <p class="text-sm text-muted-foreground">
                    Add your Snapchat export files to the queue.
                  </p>
                </div>
                <Badge
                  variant="secondary"
                  class="text-xs font-mono px-2.5 py-0.5 rounded-md"
                >
                  {session.selectedZips.length}
                </Badge>
              </div>

              <div class="flex flex-col gap-4 mt-2">
                {#if hasZipSequenceGaps}
                  <div
                    class="rounded-xl border border-destructive/30 bg-destructive/5 p-4 text-sm"
                    in:fade
                  >
                    <div class="flex items-start gap-3">
                      <AlertCircle
                        class="h-5 w-5 text-destructive shrink-0 mt-0.5"
                      />
                      <div class="min-w-0">
                        <p class="font-bold text-destructive">
                          Missing ZIP part(s)
                        </p>
                        <p
                          class="text-muted-foreground mt-1 leading-relaxed text-xs"
                        >
                          Your Snapchat export appears split into multiple
                          parts, but some part numbers are missing. Add the
                          missing ZIP(s) before starting to avoid pipeline
                          failures.
                        </p>
                        <ul
                          class="mt-2 text-xs font-mono text-muted-foreground"
                        >
                          {#each zipSequenceIssues as issue (issue.base)}
                            <li>
                              mydata~{issue.base}: missing {issue.missingParts.join(
                                ", ",
                              )}
                            </li>
                          {/each}
                        </ul>
                      </div>
                    </div>
                  </div>
                {/if}

                {#if session.selectedZips.length === 0}
                  <Button
                    variant="outline"
                    onclick={onAddZip}
                    class="w-full !px-0 py-16 flex flex-col items-center justify-center gap-5 border-dashed rounded-2xl bg-card/60 shadow-sm transition-all duration-300
                        {isDragging
                      ? 'border-primary bg-primary/5 text-primary cursor-pointer scale-[1.02]'
                      : 'border-border/60 hover:border-primary/40 hover:bg-card text-muted-foreground cursor-pointer'}"
                  >
                    <Upload class="h-8 w-8" />
                    <div class="text-center">
                      <p class="text-base font-bold text-foreground">
                        Add Memories ZIPs
                      </p>
                      <p class="text-xs mt-1.5">
                        Drop your Snapchat export files here
                      </p>
                    </div>
                  </Button>
                {:else}
                  <div class="grid grid-cols-1 gap-3">
                    {#each zipStats as zip (zip.path)}
                      <div
                        class="group relative flex items-center gap-4 p-4 bg-card border border-border/50 rounded-xl shadow-sm hover:border-primary/30 transition-colors duration-300"
                        in:slide
                      >
                        <div
                          class="h-10 w-10 rounded-lg bg-primary/10 text-primary flex items-center justify-center shrink-0 ring-1 ring-primary/10"
                        >
                          <FileArchive class="h-5 w-5" />
                        </div>
                        <div class="flex flex-col min-w-0 pr-10">
                          <span
                            class="text-sm font-bold truncate tracking-tight text-foreground/90"
                            >{zip.name}</span
                          >
                          <div
                            class="flex items-center gap-1.5 mt-1 text-[10px] font-bold uppercase tracking-wider"
                            in:fade
                          >
                            {#if session.zipValidity.get(zip.path) === "checking"}
                              <RefreshCw
                                class="h-3 w-3 animate-spin text-primary"
                              />
                              <span class="text-primary/90">Indexing...</span>
                            {:else if session.zipValidity.get(zip.path) === "valid"}
                              <CircleCheck
                                class="h-3 w-3 text-green-500 fill-green-500/10"
                              />
                              <span class="text-green-500/90">Ready</span>
                              <span class="text-muted-foreground/60 mx-1"
                                >•</span
                              >
                              <span class="text-muted-foreground/80"
                                >{zip.count} memories</span
                              >
                            {:else if session.zipValidity.get(zip.path) === "invalid"}
                              <AlertCircle
                                class="h-3 w-3 text-destructive fill-destructive/10"
                              />
                              <span class="text-destructive/90"
                                >Invalid Archive</span
                              >
                            {/if}
                          </div>
                        </div>
                        <Button
                          variant="ghost"
                          size="icon"
                          class="absolute top-1/2 -translate-y-1/2 right-3 h-8 w-8 rounded-md opacity-0 group-hover:opacity-100 hover:bg-destructive/10 hover:text-destructive transition-all duration-200"
                          onclick={() => onRemoveZip(zip.path)}
                        >
                          <X class="h-4 w-4" />
                        </Button>
                      </div>
                    {/each}
                  </div>
                  <Button
                    variant="outline"
                    onclick={onAddZip}
                    class="w-full border-dashed py-5 h-auto text-sm font-semibold gap-2 rounded-xl bg-card/60 shadow-sm hover:shadow-md transition-all duration-300 border-border/60 hover:border-primary/40 hover:bg-card text-muted-foreground cursor-pointer"
                  >
                    <Plus class="h-4 w-4" />
                    Add More
                  </Button>
                {/if}
              </div>
            </div>

            <!-- Extraction Preview (Right Column) -->
            <div class="flex flex-col gap-5">
              <div class="flex items-start justify-between">
                <div class="flex flex-col gap-1.5">
                  <h3
                    class="flex items-center gap-2 text-lg font-bold tracking-tight"
                  >
                    <Layers class="h-4.5 w-4.5 text-muted-foreground" />
                    Extraction Preview
                  </h3>
                  <p class="text-sm text-muted-foreground">
                    Summary of memories found in your ZIP archives.
                  </p>
                </div>
                <Badge
                  variant="secondary"
                  class="text-xs font-mono px-2.5 py-0.5 rounded-md"
                >
                  {previewItems.length} total
                </Badge>
              </div>

              <div class="flex flex-col gap-6 mt-2">
                <div class="grid grid-cols-2 gap-4">
                  <div
                    class="rounded-xl border border-border/60 bg-card p-4 shadow-sm flex flex-col justify-center"
                  >
                    <p
                      class="text-[11px] text-muted-foreground font-bold uppercase tracking-wider mb-1 flex items-center gap-1.5"
                    >
                      <ImageIcon class="h-3 w-3" />
                      Images
                    </p>
                    <p class="text-3xl font-black tabular-nums tracking-tight">
                      {previewImageCount}
                    </p>
                  </div>
                  <div
                    class="rounded-xl border border-border/60 bg-card p-4 shadow-sm flex flex-col justify-center"
                  >
                    <p
                      class="text-[11px] text-muted-foreground font-bold uppercase tracking-wider mb-1 flex items-center gap-1.5"
                    >
                      <Play class="h-3 w-3" />
                      Videos
                    </p>
                    <p class="text-3xl font-black tabular-nums tracking-tight">
                      {previewVideoCount}
                    </p>
                  </div>
                </div>

                <div
                  class="flex items-center justify-between gap-4 rounded-xl border border-border/60 bg-card p-4 shadow-sm"
                >
                  <div>
                    <p
                      class="text-[11px] text-muted-foreground font-bold uppercase tracking-wider"
                    >
                      Date range
                    </p>
                    <p
                      class="font-mono text-sm mt-1 border-t-0 p-0 text-foreground"
                    >
                      {previewDateRange.start
                        ? formatPreviewDate(previewDateRange.start)
                        : "—"} - {previewDateRange.end
                        ? formatPreviewDate(previewDateRange.end)
                        : "—"}
                    </p>
                  </div>
                  {#if session.hasResumeableWork}
                    <Badge
                      variant="outline"
                      class="text-[10px] bg-background font-mono px-2 opacity-80"
                    >
                      {session.remainingCount} remaining
                    </Badge>
                  {/if}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Fixed Footer for Destination & Start -->
      <div
        class="shrink-0 border-t border-border/50 bg-card/80 backdrop-blur-xl px-12 py-5 shadow-[0_-10px_40px_-15px_rgba(0,0,0,0.1)] z-20 flex flex-col sm:flex-row items-center justify-center relative min-h-[100px]"
      >
        <div
          class="max-w-5xl w-full mx-auto flex flex-col md:flex-row items-center gap-6"
        >
          <div class="flex-1 w-full relative">
            {#if !session.selectedOutput}
              <div in:fade>
                <Button
                  variant="outline"
                  onclick={onSelectOutput}
                  class="w-full group flex items-center justify-between px-6 py-8 border-dashed border-border/70 rounded-2xl bg-background/50 hover:bg-card hover:border-primary/50 text-left shadow-inner transition-all duration-300"
                >
                  <div class="flex items-center gap-4">
                    <div>
                      <h3
                        class="flex items-center gap-2 text-lg tracking-tight"
                      >
                        <FolderOpen
                          class="h-4.5 w-4.5 text-muted-foreground"
                        />
                        Select destination folder
                      </h3>
                      <p class="text-xs text-muted-foreground mt-0.5">
                        Where should we store your memories?
                      </p>
                    </div>
                  </div>
                  <div
                    class="h-8 w-8 rounded-full bg-muted/50 flex items-center justify-center group-hover:bg-primary/20 group-hover:text-primary transition-all duration-300 shadow-sm border border-border/30"
                  >
                    <Plus class="h-4 w-4" />
                  </div>
                </Button>
              </div>
            {:else}
              <div in:slide>
                <Button
                  variant="outline"
                  onclick={onSelectOutput}
                  class="w-full group flex items-center justify-between gap-4 px-6 py-8 bg-background border border-border/60 rounded-2xl ring-1 ring-black/5 dark:ring-white/5 cursor-pointer hover:bg-accent/30 hover:border-border transition-all duration-300 text-left shadow-sm"
                >
                  <div class="flex items-center gap-4 min-w-0">
                    <div class="flex flex-col min-w-0">
                      <h3 class="flex items-center gap-2 text-lg tracking-tight truncate text-foreground">
                        <FolderOpen class="h-4.5 w-4.5 text-primary shrink-0" />
                        <span class="truncate">{getFileName(session.selectedOutput)}</span>
                      </h3>
                      <p
                        class="text-xs text-muted-foreground truncate font-mono mt-0.5"
                      >
                        {session.selectedOutput}
                      </p>
                    </div>
                  </div>
                  <div
                    class="h-8 px-3 rounded-lg border border-border/50 bg-background flex items-center justify-center text-xs font-semibold text-primary group-hover:border-primary/40 group-hover:bg-primary/5 transition-all duration-300 shadow-sm shrink-0"
                  >
                    Change
                  </div>
                </Button>
              </div>
            {/if}
          </div>

          <div class="shrink-0 w-full md:w-[220px]">
            <Button
              onclick={onStartBackup}
              size="lg"
              disabled={!canStartBackup}
              class="w-full relative gap-3 font-black h-16 text-lg rounded-2xl overflow-hidden transition-all duration-300 enabled:hover:scale-[1.02] enabled:active:scale-[0.98] disabled:grayscale-[0.5] disabled:opacity-50"
            >
              {#if session.isInitializingDb}
                <RefreshCw class="h-5 w-5 animate-spin" />
                <span class="tracking-widest">INIT...</span>
              {:else if session.isParsing}
                <RefreshCw class="h-5 w-5 animate-spin" />
                <span class="tracking-widest">INDEX...</span>
              {:else}
                <div
                  class="bg-primary-foreground text-primary h-7 w-7 rounded-full flex items-center justify-center shrink-0"
                >
                  <Play class="h-3 w-3 fill-current ml-0.5" />
                </div>
                <span class="tracking-widest uppercase"
                  >{session.hasResumeableWork ? "Resume" : "Start"}</span
                >
              {/if}
            </Button>
          </div>
        </div>

        {#if session.failedCount > 0}
          <div class="absolute -top-12 z-0">
            <Button
              variant="outline"
              onclick={() => (errorsOpen = true)}
              class="h-8 text-xs gap-1.5 shadow-sm rounded-full px-4 border-destructive/20 text-destructive bg-background hover:bg-destructive/5 hover:text-destructive"
            >
              <AlertCircle class="h-3 w-3" /> Errors & Retry ({session.failedCount})
            </Button>
          </div>
        {/if}
      </div>

      <!-- Start Backup is now in the destination footer -->
    {:else if viewState === "processing"}
      <div
        class="h-full w-full flex flex-col items-center justify-center p-12 gap-12"
        in:fade
      >
        <div class="flex flex-col items-center text-center gap-4">
          <div class="relative">
            <div
              class="absolute inset-0 bg-primary/20 blur-3xl rounded-full animate-pulse"
            ></div>
            <div class="relative scale-[1.5]">
              <StatusTracker {session} />
            </div>
          </div>
          <div class="mt-8 flex flex-col items-center gap-3">
            <h2
              class="text-5xl font-black tracking-tighter tabular-nums drop-shadow-sm"
            >
              {session.progressPercentage.toFixed(1)}<span
                class="text-2xl text-muted-foreground ml-1">%</span
              >
            </h2>
            <div
              class="flex items-center gap-3 px-5 py-2.5 bg-card/40 backdrop-blur-sm rounded-xl border border-border/50 shadow-sm transition-all pulse-glow"
            >
              {#if !session.isPaused}
                <RefreshCw class="h-4 w-4 animate-spin text-primary" />
              {/if}
              <p class="text-sm font-bold tracking-tight text-foreground/80">
                {session.statusMessage || "Processing your memories..."}
              </p>
            </div>
          </div>
        </div>

        <div class="w-full max-w-md flex flex-col gap-6">
          <div class="flex flex-col items-center justify-center gap-3">
            <div class="flex items-center justify-center gap-2">
              <Button
                onclick={onTogglePause}
                variant="ghost"
                size="sm"
                class="gap-2 text-[10px] font-bold uppercase tracking-wider h-8 hover:bg-primary/5 rounded-lg"
              >
                {#if session.isPaused}
                  <Play class="h-3 w-3 fill-current" />
                  Resume
                {:else}
                  <Pause class="h-3 w-3 fill-current" />
                  Pause
                {/if}
              </Button>

              <Button
                onclick={onCancelAndReset}
                variant="ghost"
                size="sm"
                class="gap-2 text-[10px] font-bold uppercase tracking-wider h-8 hover:bg-destructive/10 hover:text-destructive rounded-lg"
              >
                <X class="h-3 w-3" />
                Cancel
              </Button>
            </div>

            {#if session.failedCount > 0}
              <div class="flex justify-center">
                <Button
                  variant="outline"
                  class="h-9 px-4 text-xs font-bold uppercase tracking-wider"
                  onclick={() => (errorsOpen = true)}
                >
                  Errors & Retry ({session.failedCount})
                </Button>
              </div>
            {/if}

            <p
              class="text-center text-[10px] text-muted-foreground font-medium animate-pulse"
            >
              Please keep the application open during this process
            </p>
          </div>
        </div>
      </div>
    {:else if viewState === "finished"}
      <div
        class="h-full w-full flex flex-col overflow-y-auto p-12 gap-12"
        in:fade
      >
        <div class="flex flex-col items-center text-center gap-6 py-12">
          <div
            class="h-24 w-24 rounded-full bg-green-500/10 text-green-500 flex items-center justify-center ring-8 ring-green-500/5"
          >
            <CheckCheck class="h-12 w-12" />
          </div>
          <div class="max-w-md">
            <h2 class="text-4xl font-black tracking-tight">
              Process Completed
            </h2>
            <p class="text-muted-foreground mt-3 text-lg">
              Your Snapchat backup has been successfully reconstructed and
              saved.
            </p>
          </div>
          <Button
            size="lg"
            onclick={handleOpenOutputFolder}
            class="gap-3 px-8 h-14 text-base font-bold rounded-xl shadow-xl shadow-primary/20"
          >
            <FolderOpen class="h-5 w-5" />
            Explore Memories
          </Button>

          {#if session.failedCount > 0}
            <div class="mt-2">
              <Button
                variant="outline"
                class="gap-2"
                onclick={() => (errorsOpen = true)}
              >
                <AlertCircle class="h-4 w-4 text-destructive" />
                Errors & Retry ({session.failedCount})
              </Button>
            </div>
          {/if}
        </div>

        <div class="flex flex-col gap-8 max-w-xl mx-auto w-full">
          <div
            class="p-8 bg-primary/5 rounded-2xl border border-primary/10 flex flex-col justify-center gap-4 text-center items-center"
          >
            <h3 class="text-xl font-bold">What's Next?</h3>
            <p class="text-sm text-muted-foreground leading-relaxed">
              You can now safely delete the original ZIPs if you wish. All your
              memories are safely organized in the destination folder.
            </p>
            <div class="flex gap-4 mt-2">
              <Button
                variant="outline"
                class="h-10 rounded-lg font-bold px-6"
                onclick={onCancelAndReset}
              >
                Start New Backup
              </Button>
            </div>
          </div>
        </div>
      </div>
    {/if}
  </main>

  <Dialog bind:open={errorsOpen}>
    <DialogContent class="sm:max-w-3xl">
      <DialogHeader>
        <DialogTitle>Errors & Retry</DialogTitle>
      </DialogHeader>

      <div class="max-h-[60vh] overflow-y-auto pr-2 space-y-3">
        {#if failedMemories.length === 0}
          <p class="text-sm text-muted-foreground">No failed items.</p>
        {:else}
          {#each failedMemories as item (item.id)}
            <div class="rounded-xl border border-border/50 bg-card/60 p-4">
              <div class="flex items-start justify-between gap-4">
                <div class="min-w-0">
                  <div class="flex items-center gap-2 flex-wrap">
                    <Badge
                      variant="outline"
                      class="text-[10px] bg-background font-mono px-2"
                    >
                      {item.mediaType}
                    </Badge>
                    <span
                      class="font-mono text-xs text-muted-foreground truncate"
                    >
                      {item.id}
                    </span>
                  </div>

                  <pre
                    class="mt-2 text-xs whitespace-pre-wrap break-words text-destructive">
                    {item.errorMessage ?? "Unknown error"}
                  </pre>
                </div>

                <div class="shrink-0 pt-1">
                  <Button
                    variant="outline"
                    size="sm"
                    class="gap-2"
                    onclick={() => onRetryMemory(item.id)}
                    disabled={session.isInitializingDb || session.isParsing}
                  >
                    <RefreshCw class="h-4 w-4" />
                    Retry
                  </Button>
                </div>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </DialogContent>
  </Dialog>
</div>
