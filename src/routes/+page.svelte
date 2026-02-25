<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { parseMemoriesJson, type ParsedMemory } from "$lib/parser";
  import confetti from "canvas-confetti";
  import { Label } from "$lib/components/ui/label";
  import {
    Card,
    CardHeader,
    CardTitle,
    CardDescription,
    CardContent,
  } from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import { Progress } from "$lib/components/ui/progress";
  import { Input } from "$lib/components/ui/input";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import {
    FolderOpen,
    Play,
    RefreshCw,
    RotateCcw,
    Video,
    Image as ImageIcon,
    CircleAlert,
    Archive,
  } from "lucide-svelte";

  let selectedZip = $state<string | null>(null);
  let selectedOutput = $state<string | null>(null);
  let parsedItems = $state<ParsedMemory[]>([]);
  let memories = $state<ParsedMemory[]>([]);
  let isProcessing = $state(false);

  // Derived state to compute progress
  let completedCount = $derived(
    memories.filter((m) => m.state === "Completed").length,
  );
  let failedCount = $derived(
    memories.filter((m) => m.state === "Failed").length,
  );
  let progressPercentage = $derived(
    memories.length > 0 ? (completedCount / memories.length) * 100 : 0,
  );
  let isAllProcessed = $derived(
    memories.length > 0 && completedCount + failedCount === memories.length,
  );

  $effect(() => {
    // Automatically stop processing spinner if everything is resolved
    if (isProcessing && isAllProcessed) {
      isProcessing = false;
      toast.success("Backup pipeline finished!");

      // Erase persistent db cache from filesystem
      invoke("cleanup_database").catch(console.error);

      // Trigger organic Confetti victory animation ONLY if all succeeded
      if (failedCount === 0) {
        const duration = 2000;
        const end = Date.now() + duration;
        const frame = () => {
          confetti({
            particleCount: 8,
            angle: 60,
            spread: 55,
            origin: { x: 0, y: 0.6 },
            colors: ["#22c55e", "#3b82f6", "#fbbf24"],
          });
          confetti({
            particleCount: 8,
            angle: 120,
            spread: 55,
            origin: { x: 1, y: 0.6 },
            colors: ["#22c55e", "#3b82f6", "#fbbf24"],
          });

          if (Date.now() < end) {
            requestAnimationFrame(frame);
          }
        };
        frame();
      } else {
        toast.warning(
          `Backup finished with ${failedCount} errors. Please check the memories marked in red.`,
        );
      }
    }
  });

  $effect(() => {
    const unlisten = listen<ParsedMemory>("memory-updated", (event) => {
      const updatedMemory = event.payload;
      // Reactively update the memory array using its ID
      const index = memories.findIndex((m) => m.id === updatedMemory.id);
      if (index !== -1) {
        memories[index] = updatedMemory;
        memories = [...memories]; // trigger re-render
      }
    });

    return () => {
      unlisten.then((f) => f());
    };
  });

  $effect(() => {
    if (
      selectedZip &&
      selectedOutput &&
      parsedItems.length > 0 &&
      memories.length === 0
    ) {
      invoke("initialize_and_load", {
        outputPath: selectedOutput,
        items: parsedItems,
      })
        .then((items) => {
          memories = items as ParsedMemory[];
          toast.success(
            `Loaded ${memories.length} memories successfully for backup!`,
          );
        })
        .catch((err) => toast.error(`DB Init Error: ${err}`));
    }
  });

  async function handleSelectZip() {
    try {
      const result = await open({
        directory: false,
        multiple: false,
        filters: [{ name: "Snapchat Data Zip", extensions: ["zip"] }],
        title: "Select Snapchat Export Zip (e.g. mydata~XXXX.zip)",
      });

      if (result === null) return;

      const path = Array.isArray(result) ? result[0] : result;
      selectedZip = path;

      const jsonContent: string = await invoke("check_zip_structure", { path });
      parsedItems = parseMemoriesJson(jsonContent);

      if (parsedItems.length === 0) {
        toast.error("No memories found in the JSON file.");
      } else {
        toast.success("Successfully Parsed Zip. Now choose an Output folder!");
      }
    } catch (err) {
      toast.error(`Error: ${err}`);
      console.error(err);
    }
  }

  async function handleSelectOutput() {
    try {
      const result = await open({
        directory: true,
        multiple: false,
        title: "Select Output Destination Folder",
      });

      if (result === null) return;
      selectedOutput = Array.isArray(result) ? result[0] : result;
    } catch (err) {
      toast.error(`Error: ${err}`);
    }
  }

  async function startBackup() {
    isProcessing = true;
    toast.info("Starting backup pipeline...");
    try {
      await invoke("start_pipeline");
    } catch (err) {
      toast.error(`Pipeline error: ${err}`);
      isProcessing = false;
    }
  }

  async function resetApp() {
    try {
      await invoke("reset_application");
      selectedZip = null;
      selectedOutput = null;
      parsedItems = [];
      memories = [];
      isProcessing = false;
      toast.info(
        "Session restarted. Please select your ZIP and Output folders again.",
      );
    } catch (err) {
      toast.error(`Reset error: ${err}`);
    }
  }

  function lazyLoadVideo(node: HTMLVideoElement, src: string) {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting && src) {
          node.src = src;
          node.load();
          observer.unobserve(node);
        }
      },
      { rootMargin: "200px" },
    );

    if (src) observer.observe(node);

    return {
      update(newSrc: string) {
        if (newSrc !== src) {
          src = newSrc;
          // Only update immediately if already intersecting/showing
          if (node.src) {
            node.src = newSrc;
            node.load();
          }
        }
      },
      destroy() {
        observer.unobserve(node);
      },
    };
  }

  function getStateColor(state: string) {
    switch (state) {
      case "Pending":
        return "bg-secondary text-black";
      case "Downloaded":
        return "bg-blue-500 text-white";
      case "Extracted":
        return "bg-orange-400 text-white";
      case "Combined":
        return "bg-yellow-400 text-black";
      case "Completed":
        return "bg-green-500 text-white";
      case "Failed":
        return "bg-destructive text-destructive";
      default:
        return "bg-secondary";
    }
  }

  function getFinalSrc(memory: ParsedMemory) {
    if (!selectedOutput) return "";
    const cleanDate = memory.originalDate
      .replace(" UTC", "")
      .replace(/:/g, "-")
      .replace(/ /g, "_");
    const ext = memory.extension || (isMaybeVideo(memory) ? "mp4" : "jpg");
    const filename = `${cleanDate}_${memory.id}.${ext}`;
    const uri = convertFileSrc(`${selectedOutput}/${filename}`);
    return uri;
  }

  function getLocalMainSrc(memory: ParsedMemory) {
    if (!selectedOutput) return "";
    const ext = memory.extension || (isMaybeVideo(memory) ? "mp4" : "jpg");
    return convertFileSrc(`${selectedOutput}/${memory.id}-main.${ext}`);
  }

  function getOverlaySrc(memory: ParsedMemory) {
    if (!selectedOutput) return "";
    return convertFileSrc(`${selectedOutput}/${memory.id}-overlay.png`);
  }

  function isMaybeVideo(memory: ParsedMemory) {
    if (memory.mediaType === "Video") return true;
    if (memory.extension) {
      return ["mp4", "mov"].includes(memory.extension.toLowerCase());
    }
    // Fallback if extension not yet known
    const url = memory.downloadUrl.toLowerCase();
    return (
      url.includes(".mp4") || url.includes(".mov") || url.includes("video")
    );
  }
</script>

<div
  class="h-screen w-full flex flex-col bg-background text-foreground overflow-hidden font-sans"
>
  <!-- Desktop-native compact header -->
  <header
    class="flex h-14 items-center justify-between border-b bg-card px-4 shrink-0"
  >
    <div class="flex items-center gap-4">
      <div class="flex flex-col">
        <h1 class="text-sm font-semibold tracking-tight leading-none">
          YellowBox
        </h1>
        <p
          class="text-[10px] text-muted-foreground uppercase tracking-wider mt-1"
        >
          Memories backup tool
        </p>
      </div>

      <!-- Quick Stats inline for compactness -->
      {#if memories.length > 0}
        <div class="h-8 w-px bg-border mx-2"></div>
        <div class="flex items-center gap-4 text-xs">
          <div class="flex flex-col items-start leading-none">
            <span class="text-muted-foreground">Total</span>
            <span class="font-medium">{memories.length}</span>
          </div>
          <div class="flex flex-col items-start leading-none">
            <span class="text-muted-foreground">Completed</span>
            <span class="font-medium text-green-500">{completedCount}</span>
          </div>
          <div class="w-24 ml-2">
            <Progress value={progressPercentage} class="h-1.5" />
          </div>
        </div>
      {/if}
    </div>

    <!-- Actions -->
    <div class="flex items-center gap-2">
      {#if selectedZip && selectedOutput && memories.length > 0}
        {#if isAllProcessed}
          <Badge
            class="bg-green-500 hover:bg-green-600 text-[11px] px-3 py-1 font-bold tracking-wider uppercase"
            >Backup Complete</Badge
          >
        {:else}
          <Button
            size="sm"
            onclick={startBackup}
            disabled={isProcessing}
            class="bg-primary text-primary-foreground hover:bg-primary/90"
          >
            {#if isProcessing}
              <RefreshCw class="mr-2 h-3.5 w-3.5 animate-spin" />
              Processing...
            {:else}
              <Play class="mr-2 h-3.5 w-3.5" />
              Start Backup
            {/if}
          </Button>
          <Button
            size="sm"
            variant="outline"
            class="border-destructive text-destructive hover:bg-destructive hover:text-destructive-foreground ml-2"
            onclick={resetApp}
            title="Restart Session"
          >
            <RotateCcw class="h-4 w-4" />
          </Button>
        {/if}
      {/if}
    </div>
  </header>

  <!-- Main Workspace -->
  <main class="flex-1 overflow-hidden bg-muted/10 relative">
    {#if selectedZip && selectedOutput && memories.length > 0}
      <!-- Dense Grid container -->
      <div class="h-full overflow-y-auto p-4 content-start">
        <div
          class="grid grid-cols-4 sm:grid-cols-5 md:grid-cols-6 lg:grid-cols-8 xl:grid-cols-10 2xl:grid-cols-12 gap-2"
        >
          {#each memories as memory}
            {@const isDone =
              memory.state === "Completed" || memory.state === "Combined"}
            {@const isExtracted = memory.state === "Extracted"}
            {@const isZip =
              memory.extension === "zip" ||
              (!memory.extension &&
                memory.downloadUrl.toLowerCase().includes(".zip"))}
            <Card
              class="group relative overflow-hidden transition-all hover:border-primary/50 hover:shadow-sm rounded-[4px] border border-border/50 p-0"
            >
              <!-- Visual Preview Placeholder -->
              <div class="aspect-[9/16] bg-black/5 relative overflow-hidden">
                {#if isMaybeVideo(memory)}
                  {@const videoSrc = isDone
                    ? getFinalSrc(memory)
                    : isExtracted
                      ? getLocalMainSrc(memory)
                      : isZip
                        ? ""
                        : `${memory.downloadUrl}#t=0.5`}
                  <video
                    use:lazyLoadVideo={videoSrc}
                    class="absolute inset-0 h-full w-full object-cover transition-all duration-700 {isDone
                      ? 'opacity-100 grayscale-0 z-10'
                      : 'opacity-0 grayscale group-hover:grayscale-0'}"
                    preload="none"
                    muted
                    playsinline
                    onloadeddata={(e) => {
                      const vid = e.currentTarget as HTMLVideoElement;
                      if (!isDone) {
                        vid.classList.remove("opacity-0");
                        vid.classList.add(
                          "opacity-50",
                          "group-hover:opacity-100",
                        );
                      }
                      if (vid.previousElementSibling && !isDone) {
                        (
                          vid.previousElementSibling as HTMLElement
                        ).style.opacity = "0";
                      }
                    }}
                    onerror={(e) => {
                      const vid = e.currentTarget as HTMLVideoElement;
                      // Retry without timestamp or fallback to remote
                      if (vid.src.includes("#t=")) {
                        vid.src = vid.src.split("#")[0];
                        vid.load();
                      } else if (
                        (isDone || isExtracted) &&
                        vid.src !== memory.downloadUrl
                      ) {
                        console.warn(
                          `Local video failed for ${memory.id}, falling back to remote`,
                        );
                        vid.src = `${memory.downloadUrl}#t=0.5`;
                        vid.load();
                      } else {
                        vid.style.display = "none";
                      }
                    }}
                  ></video>
                {:else}
                  <img
                    src={isDone
                      ? getFinalSrc(memory)
                      : isExtracted
                        ? getLocalMainSrc(memory)
                        : isZip
                          ? ""
                          : memory.downloadUrl}
                    alt="Memory"
                    class="absolute inset-0 h-full w-full object-cover transition-all duration-700 {isDone
                      ? 'opacity-100 grayscale-0 z-10'
                      : 'opacity-0 grayscale group-hover:grayscale-0'}"
                    loading="lazy"
                    onload={(e) => {
                      const img = e.currentTarget;
                      if (!isDone) {
                        img.classList.remove("opacity-0");
                        img.classList.add(
                          "opacity-50",
                          "group-hover:opacity-100",
                        );
                      }
                      if (img.previousElementSibling && !isDone) {
                        (
                          img.previousElementSibling as HTMLElement
                        ).style.opacity = "0";
                      }
                    }}
                    onerror={(e) => {
                      const img = e.currentTarget as HTMLImageElement;
                      if (
                        (isDone || isExtracted) &&
                        img.src !== memory.downloadUrl
                      ) {
                        console.warn(
                          `Local image failed for ${memory.id}, falling back to remote`,
                        );
                        img.src = memory.downloadUrl;
                      } else {
                        img.style.display = "none";
                      }
                    }}
                  />
                {/if}

                <!-- Zip Placeholder -->
                {#if isZip && !isExtracted && !isDone}
                  <div
                    class="absolute inset-0 flex flex-col items-center justify-center bg-muted/20 z-0"
                  >
                    <Archive
                      class="h-6 w-6 text-muted-foreground/30 animate-pulse"
                    />
                    <span
                      class="mt-2 text-[8px] uppercase tracking-widest text-muted-foreground/40 font-bold"
                      >Zip Bundle</span
                    >
                  </div>
                {/if}

                <!-- Overlay Preview (for Extracted state) -->
                {#if isExtracted && memory.hasOverlay}
                  <img
                    src={getOverlaySrc(memory)}
                    alt="Overlay"
                    class="absolute inset-0 h-full w-full object-contain z-20 pointer-events-none"
                    onerror={(e) => {
                      (e.currentTarget as HTMLImageElement).style.display =
                        "none";
                    }}
                  />
                {/if}

                {#if memory.errorMessage}
                  <div
                    class="absolute inset-x-0 bottom-0 top-1/2 z-30 flex flex-col justify-end bg-gradient-to-t from-red-900/90 to-red-500/10 pointer-events-auto overflow-hidden rounded-b-[4px]"
                  >
                    <div
                      class="text-white text-[9px] p-2 leading-tight font-medium max-h-full overflow-y-auto w-full"
                    >
                      <span class="font-bold flex items-center mb-1"
                        ><CircleAlert class="h-3 w-3 mr-1" /> Error</span
                      >
                      {memory.errorMessage}
                    </div>
                  </div>
                {/if}

                <Badge
                  variant="secondary"
                  class="absolute top-1.5 left-1.5 z-20 h-5 w-5 p-0 flex items-center justify-center bg-background/60 backdrop-blur-md shadow-sm border-0"
                >
                  {#if isMaybeVideo(memory)}
                    <Video class="h-3 w-3 text-foreground/80" />
                  {:else}
                    <ImageIcon class="h-3 w-3 text-foreground/80" />
                  {/if}
                </Badge>

                <div
                  class="absolute top-1.5 right-1.5 z-20 flex flex-col gap-1 items-end pointer-events-none"
                >
                  <Badge
                    variant="secondary"
                    class="text-[8px] px-1.5 py-0 h-4 border-0 shadow-sm rounded-[3px] font-medium leading-none bg-background/80 backdrop-blur-md text-foreground/80"
                  >
                    {new Date(memory.originalDate).toLocaleDateString(
                      undefined,
                      {
                        month: "short",
                        day: "numeric",
                        year: "2-digit",
                      },
                    )}
                  </Badge>
                </div>

                <div
                  class="absolute bottom-0 left-0 right-0 p-1.5 pb-1.5 flex justify-end items-end bg-gradient-to-t from-black/80 via-black/30 to-transparent z-20 pointer-events-none"
                >
                  <div class="flex flex-col gap-1 items-end">
                    <Badge
                      class="text-[9px] px-1.5 py-0 h-4 {getStateColor(
                        memory.state,
                      )} border-0 shadow-sm rounded-[3px] font-medium leading-none text-white"
                    >
                      {memory.state === "Pending" ? "Ready" : memory.state}
                    </Badge>
                  </div>
                </div>
              </div>
            </Card>
          {/each}
        </div>
      </div>
    {:else if selectedZip && selectedOutput && memories.length === 0}
      <div class="absolute inset-0 flex items-center justify-center">
        <Card class="max-w-sm rounded-xl border shadow-sm px-6 py-6">
          <CardContent
            class="flex flex-col items-center justify-center text-center p-0"
          >
            <div
              class="h-12 w-12 rounded-full bg-muted flex items-center justify-center mb-4"
            >
              <CircleAlert class="h-6 w-6 text-muted-foreground"></CircleAlert>
            </div>
            <CardTitle class="text-lg font-semibold tracking-tight"
              >No memories parsed</CardTitle
            >
            <CardDescription class="text-xs text-muted-foreground mt-1">
              The JSON file was found but no memories could be loaded. Double
              check the exported files.
            </CardDescription>
          </CardContent>
        </Card>
      </div>
    {:else}
      <div class="absolute inset-0 flex flex-col items-center justify-center">
        <Card class="w-full max-w-sm rounded-xl">
          <CardHeader class="flex flex-col items-center text-center pb-4 pt-6">
            <div
              class="h-12 w-12 rounded-xl bg-muted/50 flex items-center justify-center mb-3"
            >
              <FolderOpen class="h-6 w-6 text-muted-foreground"></FolderOpen>
            </div>
            <CardTitle class="text-lg font-semibold tracking-tight">
              Start Backup
            </CardTitle>
            <CardDescription class="text-xs text-muted-foreground mt-1">
              Select your Snapchat data and an output folder to continue.
            </CardDescription>
          </CardHeader>

          <CardContent class="flex flex-col gap-5 pt-0">
            <div class="flex flex-col gap-1.5">
              <Label for="zip-input" class="text-xs font-semibold px-0.5"
                >Snapchat Data Zip</Label
              >
              <div class="flex gap-2">
                <Input
                  id="zip-input"
                  type="text"
                  placeholder="mydata~1234.zip..."
                  value={selectedZip || ""}
                  readonly
                  class="h-9 text-xs flex-1 truncate"
                />
                <Button
                  size="sm"
                  variant="outline"
                  onclick={handleSelectZip}
                  class="shrink-0 h-9 px-3"
                >
                  Browse
                </Button>
              </div>
            </div>

            <div class="flex flex-col gap-1.5">
              <Label for="out-input" class="text-xs font-semibold px-0.5"
                >Destintation Folder</Label
              >
              <div class="flex gap-2">
                <Input
                  id="out-input"
                  type="text"
                  placeholder="C:\Backups\Snapchat..."
                  value={selectedOutput || ""}
                  readonly
                  class="h-9 text-xs flex-1 truncate"
                />
                <Button
                  size="sm"
                  variant="outline"
                  onclick={handleSelectOutput}
                  class="shrink-0 h-9 px-3"
                >
                  Browse
                </Button>
              </div>
            </div>

            <Button
              onclick={startBackup}
              disabled={isProcessing ||
                !selectedZip ||
                !selectedOutput ||
                memories.length === 0}
              class="w-full mt-2"
            >
              {#if isProcessing}
                <RefreshCw class="mr-2 h-4 w-4 animate-spin" /> Processing...
              {:else}
                <Play class="mr-2 h-4 w-4" /> Preview & Backup
              {/if}
            </Button>
          </CardContent>
        </Card>
      </div>
    {/if}
  </main>
</div>
