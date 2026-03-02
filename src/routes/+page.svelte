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
              // Don't refresh resolved paths - avoids reloading previews during backup
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
      tauriService.clearPreviewTemp(tab.id).catch(() => {});

      if (tab.failedCount === 0) {
        toast.success(`${tab.name} Backup Successful! ✨`);
        confetti({
          particleCount: 150,
          spread: 70,
          origin: { y: 0.6 },
          colors: ["#FFFC00", "#ffffff", "#000000"], // Snapchat colors
        });
      } else {
        toast.warning(
          `${tab.name} finished with ${tab.failedCount} error(s).`,
          {
            description:
              "Some items could not be backed up. Click 'Retry' on red items to fix.",
            duration: 6000,
          },
        );
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
          // Don't re-resolve paths - keeps existing temp previews, no reload
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

        // Extract media to temp folder for local previews (no CDN)
        const ids = tab.parsedItems.map((m) => m.id);
        try {
          await tauriService.extractPreviewMedia(tab.id, path, ids);
          const paths = await tauriService.resolveLocalMediaPaths(tab.id, ids);
          tab.resolvedLocalPaths = paths;
        } catch (e) {
          console.warn("Preview extraction failed, will use remote URLs:", e);
        }
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
      await tauriService.startPipeline(tab.id, appConfig.overwriteExisting);
    } catch (err) {
      toast.error(`Pipeline error: ${err}`);
      tab.isProcessing = false;
    }
  }

  async function togglePause() {
    const tab = activeTab;
    if (!tab) return;

    if (tab.isPaused) {
      toast.info(`Resuming backup pipeline for ${tab.name}...`);
      tab.isProcessing = true;
      try {
        await tauriService.startPipeline(tab.id, false); // Resume doesn't overwrite
        tab.isPaused = false;
      } catch (err) {
        toast.error(`Pipeline resume error: ${err}`);
      }
    } else {
      toast.info(`Pausing backup pipeline for ${tab.name}...`);
      try {
        await tauriService.pausePipeline(tab.id);
        tab.isPaused = true;
        tab.isProcessing = false;
      } catch (err) {
        toast.error(`Pipeline pause error: ${err}`);
      }
    }
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
          session={activeTab}
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
            sessionId={activeTab.id}
            memories={activeTab.memories.length > 0
              ? activeTab.memories
              : activeTab.parsedItems}
            selectedOutput={activeTab.selectedOutput}
            resolvedLocalPaths={activeTab.resolvedLocalPaths}
            isProcessing={activeTab.isProcessing}
            isAllProcessed={activeTab.isAllProcessed}
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
