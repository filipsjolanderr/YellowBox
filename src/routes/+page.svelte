<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { parseMemoriesJson, type ParsedMemory } from "$lib/parser";
  import confetti from "canvas-confetti";
  import { toast } from "svelte-sonner";
  import { tauriService } from "$lib/services/tauri";
  import { appConfig } from "$lib/config.svelte";
  import { fade } from "svelte/transition";
  import { tweened, type Tweened } from "svelte/motion";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  import Header from "$lib/components/Header.svelte";
  import StatsPanel from "$lib/components/StatsPanel.svelte";
  import SetupCard from "$lib/components/SetupCard.svelte";
  import MemoryGrid from "$lib/components/MemoryGrid.svelte";
  import { Session } from "$lib/session.svelte";

  let tabs = $state<Session[]>([]);
  let activeTabId = $state<string>("");
  let activeTab = $derived(tabs.find((t) => t.id === activeTabId) || null);

  let hasAttemptedAppLoad = $state(false);

  let listeners = new Map<string, UnlistenFn>();

  // Extract store for Svelte auto-sub handling ($progressStore)
  let dummyProgress = tweened(0);
  let progressStore = $derived(
    activeTab ? activeTab.parsingProgress : dummyProgress,
  );

  $effect(() => {
    if (tabs.length === 0) {
      handleNewTab();
    }
  });

  $effect(() => {
    // Add missing listeners
    tabs.forEach((tab) => {
      if (!listeners.has(tab.id)) {
        const p = tauriService.listenForMemoryUpdates(
          tab.id,
          (updatedMemory) => {
            const index = tab.memories.findIndex(
              (m) => m.id === updatedMemory.id,
            );
            if (index !== -1) {
              tab.memories[index] = updatedMemory;
              tab.memories = [...tab.memories];
            }
          },
        );
        p.then((fn) => listeners.set(tab.id, fn));
      }
    });

    // Cleanup abandoned listeners
    const currentIds = new Set(tabs.map((t) => t.id));
    for (const [id, fn] of listeners.entries()) {
      if (!currentIds.has(id)) {
        fn();
        listeners.delete(id);
      }
    }
  });

  function handleNewTab() {
    const id = crypto.randomUUID();
    const num = tabs.length > 0 ? tabs.length + 1 : 1;
    const tab = new Session(id, `Backup ${num}`);
    tabs.push(tab);
    activeTabId = id;
  }

  function handleCloseTab(id: string) {
    tauriService.closeSession(id).catch(console.error);
    const index = tabs.findIndex((t) => t.id === id);
    if (index !== -1) {
      tabs.splice(index, 1);
      if (activeTabId === id && tabs.length > 0) {
        activeTabId = tabs[Math.max(0, index - 1)].id;
      } else if (tabs.length === 0) {
        handleNewTab(); // Always enforce at least 1 tab visually
      }
    }
  }

  // Monitor active tab completion and emit fireworks + unmark processing processing
  $effect(() => {
    // Since we mutate activeTab, assign it locally
    const tab = activeTab;
    if (!tab) return;

    if (tab.isProcessing && tab.isAllProcessed) {
      tab.isProcessing = false;
      toast.success(`${tab.name} finished!`);

      if (tab.failedCount === 0) {
        confetti({ particleCount: 16, spread: 55, origin: { x: 0.5, y: 1 } });
      } else {
        toast.warning(`${tab.name} finished with error(s). Check red markers.`);
      }
    }
  });

  // Initial app load config hydration on first tab strictly
  $effect(() => {
    const tab = tabs[0];
    if (
      tabs.length === 1 &&
      tab &&
      !hasAttemptedAppLoad &&
      appConfig.lastZip &&
      appConfig.lastOutput
    ) {
      hasAttemptedAppLoad = true;
      tab.hasAttemptedLoad = true;
      tab.selectedZip = appConfig.lastZip;
      tab.selectedOutput = appConfig.lastOutput;

      tauriService
        .checkZipStructure(tab.id, tab.selectedZip)
        .then((content) => {
          tab.parsedItems = parseMemoriesJson(content);
        })
        .catch((err) => {
          console.error("Auto reload zip fail", err);
          tab.selectedZip = null;
          tab.selectedOutput = null;
          appConfig.lastZip = null;
          appConfig.lastOutput = null;
          appConfig.save();
        });
    }
  });

  // Database initialization logic for Active Tab
  $effect(() => {
    const tab = activeTab;
    if (!tab) return;
    if (
      tab.selectedZip &&
      tab.selectedOutput &&
      tab.parsedItems.length > 0 &&
      tab.memories.length === 0
    ) {
      tauriService
        .initializeAndLoad(tab.id, tab.selectedOutput, tab.parsedItems)
        .then((items) => {
          tab.memories = items as ParsedMemory[];
          toast.success(
            `Loaded ${tab.memories.length} memories into ${tab.name}!`,
          );
        })
        .catch((err) => toast.error(`DB Init Error: ${err}`));
    }
  });

  async function processZipPath(path: string) {
    const tab = activeTab;
    if (!tab) return;

    try {
      tab.isParsingZip = true;
      tab.parsingProgress.set(0, { duration: 0 });

      await new Promise((r) => setTimeout(r, 600));

      tab.parsingProgress.set(85, { duration: 2500 });
      tab.selectedZip = path;

      const jsonContent = await tauriService.checkZipStructure(tab.id, path);

      tab.parsingProgress.set(95, { duration: 300 });
      tab.parsedItems = parseMemoriesJson(jsonContent);

      if (tab.parsedItems.length === 0) {
        toast.error("No memories found in JSON array.");
        tab.isParsingZip = false;
        tab.selectedZip = null;
      } else {
        await tab.parsingProgress.set(100, { duration: 500 });
        await new Promise((r) => setTimeout(r, 800));

        toast.success(`${tab.name} Loaded Zip.`);
        appConfig.lastZip = path;
        appConfig.save();
        tab.isParsingZip = false;
      }
    } catch (err) {
      toast.error(`Error processing zip: ${err}`);
      tab.selectedZip = null;
      tab.isParsingZip = false;
    }
  }

  async function handleSelectZip() {
    try {
      const result = await open({
        directory: false,
        multiple: false,
        filters: [{ name: "Snapchat Data Zip", extensions: ["zip"] }],
        title: "Select Snapchat Export Zip",
      });

      if (result) {
        await processZipPath(Array.isArray(result) ? result[0] : result);
      }
    } catch (err) {
      toast.error(`Dialog error: ${err}`);
      console.error(err);
    }
  }

  async function handleSelectOutput() {
    const tab = activeTab;
    if (!tab) return;

    try {
      const result = await open({
        directory: true,
        multiple: false,
        title: "Select Output Destination Folder",
      });

      if (result) {
        tab.selectedOutput = Array.isArray(result) ? result[0] : result;
        appConfig.lastOutput = tab.selectedOutput;
        appConfig.save();
      }
    } catch (err) {
      toast.error(`Directory Error: ${err}`);
    }
  }

  async function startBackup() {
    const tab = activeTab;
    if (!tab) return;

    tab.isProcessing = true;
    toast.info(`Starting backup pipeline for ${tab.name}...`);
    try {
      await tauriService.startPipeline(
        tab.id,
        appConfig.concurrencyLimit,
        appConfig.overwriteExisting,
      );
    } catch (err) {
      toast.error(`Pipeline error: ${err}`);
      tab.isProcessing = false;
    }
  }

  function togglePause() {
    const tab = activeTab;
    if (!tab) return;
    tab.isPaused = !tab.isPaused;
    toast.info(tab.isPaused ? `${tab.name} Paused` : `${tab.name} Resumed`);
  }
</script>

<div
  class="h-screen w-full flex flex-col bg-background text-foreground overflow-hidden font-sans"
>
  <Header
    {tabs}
    {activeTabId}
    onTabChange={(id) => (activeTabId = id)}
    onNewTab={handleNewTab}
    onCloseTab={handleCloseTab}
  />

  {#if activeTab}
    {#if activeTab.selectedZip && (activeTab.memories.length > 0 || activeTab.parsedItems.length > 0) && !activeTab.isParsingZip}
      <div in:fade={{ duration: 300, delay: 150 }}>
        <StatsPanel
          memories={activeTab.memories}
          memoriesLength={activeTab.totalCount}
          completedCount={activeTab.completedCount}
          progressPercentage={activeTab.progressPercentage}
          isAllProcessed={activeTab.isAllProcessed}
          selectedZip={activeTab.selectedZip}
          selectedOutput={activeTab.selectedOutput}
          isProcessing={activeTab.isProcessing}
          isPaused={activeTab.isPaused}
          onSelectOutput={handleSelectOutput}
          onStartBackup={startBackup}
          onTogglePause={togglePause}
        />
      </div>
    {/if}

    <main class="flex-1 overflow-hidden bg-muted/10 relative">
      {#if activeTab.selectedZip && (activeTab.memories.length > 0 || activeTab.parsedItems.length > 0) && !activeTab.isParsingZip}
        <div
          in:fade={{ duration: 300, delay: 300 }}
          out:fade={{ duration: 300 }}
          class="absolute inset-0"
        >
          <MemoryGrid
            memories={activeTab.memories.length > 0
              ? activeTab.memories
              : activeTab.parsedItems}
            selectedOutput={activeTab.selectedOutput}
          />
        </div>
      {:else}
        <div
          in:fade={{ duration: 300, delay: 300 }}
          out:fade={{ duration: 300 }}
          class="absolute inset-0 flex items-center justify-center"
        >
          <SetupCard
            selectedZip={activeTab.selectedZip}
            isParsing={activeTab.isParsingZip}
            progressValue={$progressStore}
            onSelectZip={handleSelectZip}
            onDropZip={processZipPath}
          />
        </div>
      {/if}
    </main>
  {/if}
</div>
