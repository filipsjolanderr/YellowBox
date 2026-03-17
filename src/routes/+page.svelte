<script lang="ts">
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { parseMemoriesJson, type ParsedMemory } from "$lib/parser";
  import confetti from "canvas-confetti";
  import { toast } from "svelte-sonner";
  import { tauriService } from "$lib/services/tauri";
  import { appConfig } from "$lib/config.svelte";
  import { tweened, type Tweened } from "svelte/motion";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  import Header from "$lib/components/Header.svelte";

  import ProcessingView from "$lib/components/ProcessingView.svelte";
  import { Session } from "$lib/session.svelte";
  import { fade } from "svelte/transition";

  let tabs = $state<Session[]>([]);
  let activeTabId = $state<string>("");
  let activeTab = $derived(tabs.find((t) => t.id === activeTabId) || null);

  let hasAttemptedAppLoad = $state(false);

  let listeners = new Map<string, UnlistenFn>();

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
    tabs.forEach((tab) => {
      if (!listeners.has(tab.id)) {
        const p = tauriService.listenForMemoryUpdates(tab.id, (updatedMemory) => {
          const index = tab.memories.findIndex((m) => m.id === updatedMemory.id);
          if (index !== -1) {
            tab.memories[index] = updatedMemory;
            tab.memories = [...tab.memories];
          }
        });
        p.then((fn) => listeners.set(tab.id, fn));
      }
    });

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

  async function handleCloseTab(id: string) {
    const tab = tabs.find((t) => t.id === id);
    if (tab?.isProcessing) {
      const confirmed = await ask(
        `Backup "${tab.name}" is currently in progress. Are you sure you want to close this tab?`,
        { title: "Confirm Close Tab", kind: "warning" },
      );
      if (!confirmed) return;
    }

    tauriService.closeSession(id).catch(console.error);
    const index = tabs.findIndex((t) => t.id === id);
    if (index !== -1) {
      tabs.splice(index, 1);
      if (activeTabId === id && tabs.length > 0) {
        activeTabId = tabs[Math.max(0, index - 1)].id;
      } else if (tabs.length === 0) {
        handleNewTab();
      }
    }
  }

  // Completion detection
  $effect(() => {
    const tab = activeTab;
    if (!tab) return;

    if (tab.isProcessing && tab.isAllProcessed) {
      tab.isProcessing = false;
      if (tab.failedCount === 0) {
        toast.success(`${tab.name} successful!`);
        confetti({
          particleCount: 150,
          spread: 70,
          origin: { y: 0.6 },
          colors: ["#FFFC00", "#ffffff", "#000000"],
        });
      } else {
        toast.warning(`${tab.name} finished with ${tab.failedCount} error(s).`, {
          description: "Some items could not be backed up.",
          duration: 6000,
        });
      }
    }
  });

  // Auto-load saved config on first tab
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
      tab.selectedZips = [appConfig.lastZip];
      tab.selectedOutput = appConfig.lastOutput;

      tab.isParsing = true;
      tab.parsingProgress.set(50, { duration: 1000 });

      tauriService
        .checkZipStructure(tab.id, appConfig.lastZip)
        .then((content) => {
          if (content) {
            tab.parsedItems = parseMemoriesJson(content);
          }
          tab.isParsing = false;
        })
        .catch((err) => {
          console.error("Auto reload zip fail", err);
          tab.selectedZips = [];
          tab.selectedOutput = null;
          appConfig.lastZip = null;
          appConfig.lastOutput = null;
          appConfig.save();
          tab.isParsing = false;
        });
    }
  });

  // DB init once zips are parsed
  $effect(() => {
    const tab = activeTab;
    if (!tab) return;
    if (
      tab.selectedZips.length > 0 &&
      tab.selectedOutput &&
      tab.parsedItems.length > 0 &&
      tab.memories.length === 0 &&
      !tab.isInitializingDb &&
      !tab.isParsing
    ) {
      tab.isInitializingDb = true;
      tab.parsingProgress.set(80, { duration: 1000 });

      tauriService
        .initializeAndLoad(tab.id, tab.selectedOutput!, tab.parsedItems)
        .then(async (items) => {
          tab.memories = items as ParsedMemory[];
          await tab.parsingProgress.set(100, { duration: 500 });
        })
        .catch((err) => {
          toast.error(`DB Init error: ${err}`);
        })
        .finally(() => {
          tab.isInitializingDb = false;
        });
    }
  });

  async function processZipPath(path: string) {
    const tab = activeTab;
    if (!tab) return;
    try {
      tab.isParsing = true;
      tab.parsingProgress.set(0, { duration: 0 });
      tab.parsingProgress.set(85, { duration: 2500 });

      if (!tab.selectedZips.includes(path)) {
        tab.selectedZips.push(path);
      }

      const jsonContent = await tauriService.checkZipStructure(tab.id, path);
      tab.parsingProgress.set(95, { duration: 300 });

      let newItemsCount = 0;
      if (jsonContent) {
        const newItems = parseMemoriesJson(jsonContent);
        newItemsCount = newItems.length;
        const existingIds = new Set(tab.parsedItems.map((i) => i.id));
        const mergedItems = [...tab.parsedItems];
        for (const item of newItems) {
          if (!existingIds.has(item.id)) {
            mergedItems.push(item);
            existingIds.add(item.id);
          }
        }
        tab.parsedItems = mergedItems;
      }

      if (tab.parsedItems.length === 0 && !jsonContent) {
        toast.info(`Added ${path.split(/[\\/]/).pop()} (no memories found)`);
        appConfig.lastZip = path;
        appConfig.save();
        await tab.parsingProgress.set(100, { duration: 500 });
        tab.isParsing = false;
      } else if (tab.parsedItems.length === 0) {
        toast.error("No memories found in JSON.");
        tab.isParsing = false;
        tab.selectedZips = tab.selectedZips.filter((z) => z !== path);
      } else {
        appConfig.lastZip = path;
        appConfig.save();
        await tab.parsingProgress.set(100, { duration: 500 });
        tab.isParsing = false;
        if (newItemsCount > 0) {
            toast.success(`Loaded ${newItemsCount.toLocaleString()} memories from ${path.split(/[\\/]/).pop()}`);
        } else {
            toast.success(`Added ${path.split(/[\\/]/).pop()}`);
        }
      }
    } catch (err) {
      toast.error(`Error processing zip: ${err}`);
      tab.selectedZips = tab.selectedZips.filter((z) => z !== path);
      tab.isParsing = false;
    }
  }

  function removeZip(path: string) {
    const tab = activeTab;
    if (!tab) return;
    tab.selectedZips = tab.selectedZips.filter((z) => z !== path);
    if (tab.selectedZips.length === 0) {
      tab.parsedItems = [];
      tab.memories = [];
    }
  }

  async function handleSelectZip() {
    try {
      const result = await open({
        directory: false,
        multiple: true,
        filters: [{ name: "Snapchat Data Zip", extensions: ["zip"] }],
        title: "Select Snapchat Export Zip(s)",
      });
      if (result) {
        const paths = Array.isArray(result) ? result : [result];
        for (const p of paths) {
          await processZipPath(p);
        }
      }
    } catch (err) {
      toast.error(`Dialog error: ${err}`);
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
      toast.error(`Directory error: ${err}`);
    }
  }

  async function startBackup() {
    const tab = activeTab;
    if (!tab) return;
    tab.isProcessing = true;
    toast.info(`Starting ${tab.name}...`);
    try {
      await tauriService.startPipeline(
        tab.id,
        appConfig.overwriteExisting,
        appConfig.maxConcurrency,
        tab.selectedOutput,
      );
    } catch (err) {
      toast.error(`Pipeline error: ${err}`);
      tab.isProcessing = false;
    }
  }

  async function togglePause() {
    const tab = activeTab;
    if (!tab) return;
    if (tab.isPaused) {
      toast.info(`Resuming ${tab.name}...`);
      tab.isProcessing = true;
      try {
        await tauriService.startPipeline(tab.id, false, appConfig.maxConcurrency, tab.selectedOutput);
        tab.isPaused = false;
      } catch (err) {
        toast.error(`Resume error: ${err}`);
      }
    } else {
      toast.info(`Pausing ${tab.name}...`);
      try {
        await tauriService.pausePipeline(tab.id);
        tab.isPaused = true;
        tab.isProcessing = false;
      } catch (err) {
        toast.error(`Pause error: ${err}`);
      }
    }
  }


</script>

<div class="h-screen w-full flex flex-col bg-background text-foreground overflow-hidden font-sans">
  <Header
    {tabs}
    {activeTabId}
    onTabChange={(id) => (activeTabId = id)}
    onNewTab={handleNewTab}
    onCloseTab={handleCloseTab}
  />

  {#if activeTab}
    <main class="flex-1 overflow-hidden relative">
      <!-- Always-visible ProcessingView -->
      <div class="absolute inset-0" in:fade={{ duration: 200 }}>
        <ProcessingView
          session={activeTab}
          onSelectOutput={handleSelectOutput}
          onStartBackup={startBackup}
          onTogglePause={togglePause}
          onAddZip={handleSelectZip}
          onDropZip={processZipPath}
          onRemoveZip={removeZip}
        />
      </div>

    </main>
  {/if}
</div>
