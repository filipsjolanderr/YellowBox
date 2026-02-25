<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { parseMemoriesJson, type ParsedMemory } from "$lib/parser";
  import confetti from "canvas-confetti";
  import {
    Card,
    CardTitle,
    CardDescription,
    CardContent,
  } from "$lib/components/ui/card";
  import { toast } from "svelte-sonner";
  import { CircleAlert } from "lucide-svelte";
  import { tauriService } from "$lib/services/tauri";
  import { appConfig } from "$lib/config.svelte";
  import { tweened } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { fade } from "svelte/transition";

  import Header from "$lib/components/Header.svelte";
  import SetupCard from "$lib/components/SetupCard.svelte";
  import MemoryGrid from "$lib/components/MemoryGrid.svelte";

  let selectedZip = $state<string | null>(null);
  let selectedOutput = $state<string | null>(null);
  let parsedItems = $state<ParsedMemory[]>([]);
  let memories = $state<ParsedMemory[]>([]);
  let isProcessing = $state(false);
  let hasAttemptedLoad = $state(false);

  let isParsingZip = $state(false);
  let parsingProgress = tweened(0, { duration: 400, easing: cubicOut });

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
      tauriService.cleanupDatabase().catch(console.error);

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
    let unlistenFn: (() => void) | undefined;
    tauriService
      .listenForMemoryUpdates((updatedMemory) => {
        // Reactively update the memory array using its ID
        const index = memories.findIndex((m) => m.id === updatedMemory.id);
        if (index !== -1) {
          memories[index] = updatedMemory;
          memories = [...memories]; // trigger re-render
        }
      })
      .then((fn) => {
        unlistenFn = fn;
      });

    return () => {
      if (unlistenFn) unlistenFn();
    };
  });

  $effect(() => {
    // Initial Hydration from AppConfig
    if (!hasAttemptedLoad && appConfig.lastZip && appConfig.lastOutput) {
      hasAttemptedLoad = true;
      selectedZip = appConfig.lastZip;
      selectedOutput = appConfig.lastOutput;

      tauriService
        .checkZipStructure(selectedZip)
        .then((jsonContent) => {
          parsedItems = parseMemoriesJson(jsonContent);
        })
        .catch((err) => {
          console.error("Failed to auto-reload zip", err);
          selectedZip = null;
          selectedOutput = null;
          appConfig.lastZip = null;
          appConfig.lastOutput = null;
          appConfig.save();
        });
    }
  });

  $effect(() => {
    if (
      selectedZip &&
      selectedOutput &&
      parsedItems.length > 0 &&
      memories.length === 0
    ) {
      tauriService
        .initializeAndLoad(selectedOutput, parsedItems)
        .then((items) => {
          memories = items as ParsedMemory[];
          toast.success(
            `Loaded ${memories.length} memories successfully for backup!`,
          );
        })
        .catch((err) => toast.error(`DB Init Error: ${err}`));
    }
  });

  async function processZipPath(path: string) {
    try {
      isParsingZip = true;
      parsingProgress.set(0, { duration: 0 });

      // Small artificial delay before showing fake progress
      await new Promise((r) => setTimeout(r, 600));

      parsingProgress.set(85, { duration: 2500 }); // fake loading progress

      selectedZip = path;
      const jsonContent = await tauriService.checkZipStructure(path);

      parsingProgress.set(95, { duration: 300 });
      parsedItems = parseMemoriesJson(jsonContent);

      if (parsedItems.length === 0) {
        toast.error("No memories found in the JSON file.");
        isParsingZip = false;
        selectedZip = null;
      } else {
        await parsingProgress.set(100, { duration: 500 });

        // Artificial delay at 100% so user sees completion before sudden jump
        await new Promise((r) => setTimeout(r, 800));

        toast.success("Successfully Parsed Zip.");
        appConfig.lastZip = path;
        appConfig.save();
        isParsingZip = false;
      }
    } catch (err) {
      toast.error(`Error processing zip: ${err}`);
      console.error(err);
      selectedZip = null;
      isParsingZip = false;
    }
  }

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
      await processZipPath(path);
    } catch (err) {
      toast.error(`Error opening file dialog: ${err}`);
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
      appConfig.lastOutput = selectedOutput;
      appConfig.save();
    } catch (err) {
      toast.error(`Error: ${err}`);
    }
  }

  async function startBackup() {
    isProcessing = true;
    toast.info("Starting backup pipeline...");
    try {
      await tauriService.startPipeline(
        appConfig.concurrencyLimit,
        appConfig.overwriteExisting,
      );
    } catch (err) {
      toast.error(`Pipeline error: ${err}`);
      isProcessing = false;
    }
  }

  async function resetApp() {
    try {
      await tauriService.resetApplication();
      selectedZip = null;
      selectedOutput = null;
      parsedItems = [];
      memories = [];
      isProcessing = false;
      appConfig.lastZip = null;
      appConfig.lastOutput = null;
      appConfig.save();
      toast.info(
        "Session restarted. Please select your ZIP and Output folders again.",
      );
    } catch (err) {
      toast.error(`Reset error: ${err}`);
    }
  }
</script>

<div
  class="h-screen w-full flex flex-col bg-background text-foreground overflow-hidden font-sans"
>
  <Header
    memoriesLength={memories.length > 0 ? memories.length : parsedItems.length}
    {completedCount}
    {progressPercentage}
    {isAllProcessed}
    {selectedZip}
    {selectedOutput}
    {isProcessing}
    onSelectOutput={handleSelectOutput}
    onStartBackup={startBackup}
    onResetApp={resetApp}
  />

  <main class="flex-1 overflow-hidden bg-muted/10 relative">
    {#if selectedZip && (memories.length > 0 || parsedItems.length > 0) && !isParsingZip}
      <div
        in:fade={{ duration: 300, delay: 300 }}
        out:fade={{ duration: 300 }}
        class="absolute inset-0"
      >
        <MemoryGrid
          memories={memories.length > 0 ? memories : parsedItems}
          {selectedOutput}
        />
      </div>
    {:else}
      <div
        in:fade={{ duration: 300, delay: 300 }}
        out:fade={{ duration: 300 }}
        class="absolute inset-0 flex items-center justify-center"
      >
        <SetupCard
          {selectedZip}
          isParsing={isParsingZip}
          progressValue={$parsingProgress}
          onSelectZip={handleSelectZip}
          onDropZip={processZipPath}
        />
      </div>
    {/if}
  </main>
</div>
