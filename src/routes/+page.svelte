<script lang="ts">
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { parseMemoriesJson, type ParsedMemory } from "$lib/parser";
  import confetti from "canvas-confetti";
  import { tauriService } from "$lib/services/tauri";
  import { appConfig } from "$lib/config.svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  import Header from "$lib/components/Header.svelte";

  import ProcessingView from "$lib/components/ProcessingView.svelte";
  import { Session } from "$lib/session.svelte";
  import { fade } from "svelte/transition";

  // If we have a previously-running session recorded, reuse the same session_id so
  // the backend can load the existing SQLite DB and continue from where it left off.
  let session = $state<Session>(
    new Session(appConfig.lastSessionId ?? crypto.randomUUID(), "Backup"),
  );

  let hasAttemptedAppLoad = $state(false);

  let unlistenAll: UnlistenFn | null = null;
  let unlistenStatus: UnlistenFn | null = null;
  let unlistenZip: UnlistenFn | null = null;

  let dbInitToken = $state(0);

  async function attachListeners(sessionId: string) {
    [unlistenAll, unlistenStatus, unlistenZip].forEach((fn) => fn?.());
    unlistenAll = await tauriService.listenForMemoryUpdates(sessionId, (updatedMemory) => {
      const index = session.memories.findIndex((m) => m.id === updatedMemory.id);
      if (index !== -1) {
      session.memories[index] = updatedMemory;
      }
    });
    unlistenStatus = await tauriService.listenForPipelineStatus(sessionId, (status) => {
      session.statusMessage = status;
    });
    unlistenZip = await tauriService.listenForZipIndexingProgress(sessionId, (payload) => {
      session.zipProgress.set(payload.path, payload.progress);
    });
  }

  onMount(() => {
    attachListeners(session.id).catch(console.error);
    return () => {
      [unlistenAll, unlistenStatus, unlistenZip].forEach((fn) => fn?.());
    };
  });

  // Completion detection
  $effect(() => {
    if (session.isProcessing && session.isAllProcessed) {
      session.isProcessing = false;
      // Backup is done; no resume needed.
      appConfig.lastSessionId = null;
      appConfig.save();
      if (session.failedCount === 0) {
        confetti({
          particleCount: 150,
          spread: 70,
          origin: { y: 0.6 },
          colors: ["#FFFC00", "#ffffff", "#000000"],
        });
      } else {
        // no toast; UI already shows failed count
      }
    }
  });

  // Auto-load saved config on first run
  $effect(() => {
    if (
      !hasAttemptedAppLoad &&
      appConfig.lastZips.length > 0 &&
      appConfig.lastOutput
    ) {
      hasAttemptedAppLoad = true;
      session.hasAttemptedLoad = true;
      session.selectedOutput = appConfig.lastOutput;

      // Start loading each ZIP sequentially
      (async () => {
        session.activeParsingTasks++;
        try {
          for (const zipPath of appConfig.lastZips) {
            await processZipPath(zipPath);
          }
        } finally {
          session.activeParsingTasks--;
        }
      })();
    }
  });

  // DB init once zips are parsed
  $effect(() => {
    if (
      session.selectedZips.length > 0 &&
      session.selectedOutput &&
      session.parsedItems.length > 0 &&
      session.memories.length === 0 &&
      !session.isInitializingDb &&
      !session.isParsing
    ) {
      const token = ++dbInitToken;
      const outputAtStart = session.selectedOutput;
      session.isInitializingDb = true;
      session.parsingProgress.set(80, { duration: 1000 });

      tauriService
        .initializeAndLoad(session.id, session.selectedOutput!, session.parsedItems)
        .then(async (items) => {
          // If destination changed mid-init, ignore outdated results.
          if (token !== dbInitToken || session.selectedOutput !== outputAtStart) return;
          session.memories = items as ParsedMemory[];
          await session.parsingProgress.set(100, { duration: 500 });
        })
        .catch((err) => {
          console.error("DB Init error:", err);
        })
        .finally(() => {
          if (token === dbInitToken) {
            session.isInitializingDb = false;
          }
        });
    }
  });

  function syncValidExportPaths() {
    const valid = session.selectedZips.filter((p) => session.zipValidity.get(p) === "valid");
    tauriService.setExportPaths(session.id, valid).catch(console.error);
  }

  async function processZipPath(path: string) {
    try {
      session.activeParsingTasks++;
      session.parsingProgress.set(0, { duration: 0 });
      session.parsingProgress.set(85, { duration: 2500 });
      
      // Initialize progress for this specific zip
      session.zipProgress.set(path, 0);
      session.zipValidity.set(path, "checking");

      if (!session.selectedZips.includes(path)) {
        session.selectedZips.push(path);
      }

      // Retry mechanism for ZIP loading
      let newItems: ParsedMemory[] = [];
      let lastErr: any = null;
      for (let attempt = 1; attempt <= 3; attempt++) {
        try {
          // Update progress slightly per attempt if it's taking time
          session.zipProgress.set(path, (attempt - 1) * 20);
          newItems = await tauriService.checkZipStructure(session.id, path);
          session.zipProgress.set(path, 80);
          break;
        } catch (err) {
          lastErr = err;
          if (attempt < 3) {
            console.warn(`Retry ${attempt}/3 for ${path}: ${err}`);
            await new Promise((resolve) => setTimeout(resolve, 1000 * attempt));
          }
        }
      }

      if (lastErr && newItems.length === 0) {
        session.zipValidity.set(path, "invalid");
        session.zipProgress.set(path, 100);
        session.memoryIdsByZip.delete(path);
        syncValidExportPaths();
        return;
      }

      session.parsingProgress.set(95, { duration: 300 });

      if (newItems.length > 0) {
        const existingIds = new Set(session.parsedItems.map((i) => i.id));
        const mergedItems = [...session.parsedItems];
        const addedIds: string[] = [];
        for (const item of newItems) {
          if (!existingIds.has(item.id)) {
            mergedItems.push(item);
            existingIds.add(item.id);
            addedIds.push(item.id);
          }
        }
        session.parsedItems = mergedItems;
        // Only "newly added" IDs count for per-ZIP preview totals,
        // ensuring the counts sum up to the unique set we keep in state.
        session.memoryIdsByZip.set(path, addedIds);
      }

      // Valid Snapchat export must produce at least one memory item from the ZIP JSON.
      if (newItems.length === 0) {
        session.zipValidity.set(path, "invalid");
        session.zipProgress.set(path, 100);
        session.memoryIdsByZip.delete(path);
        await session.parsingProgress.set(100, { duration: 500 });
        syncValidExportPaths();
      } else {
        session.zipValidity.set(path, "valid");
        if (!appConfig.lastZips.includes(path)) {
          appConfig.lastZips.push(path);
          appConfig.save();
        }
        session.zipProgress.set(path, 100);
        await session.parsingProgress.set(100, { duration: 500 });
        syncValidExportPaths();
      }
    } catch (err) {
      console.error("Error processing zip:", err);
      session.zipValidity.set(path, "invalid");
      session.zipProgress.set(path, 100);
      session.memoryIdsByZip.delete(path);
      syncValidExportPaths();
    } finally {
      session.activeParsingTasks--;
    }
  }

  function removeZip(path: string) {
    session.selectedZips = session.selectedZips.filter((z) => z !== path);
    appConfig.lastZips = appConfig.lastZips.filter((z) => z !== path);
    appConfig.save();
    session.zipProgress.delete(path);
    session.zipValidity.delete(path);
    session.memoryIdsByZip.delete(path);
    syncValidExportPaths();

    if (session.selectedZips.length === 0) {
      session.parsedItems = [];
      session.memories = [];
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
        
        // Use a persistent counter for the whole batch to prevent premature DB init
        session.activeParsingTasks++; 
        try {
          for (const p of paths) {
            await processZipPath(p);
          }
        } finally {
          session.activeParsingTasks--;
        }
      }
    } catch (err) {
      console.error("Dialog error:", err);
    }
  }

  async function handleSelectOutput() {
    try {
      const result = await open({
        directory: true,
        multiple: false,
        title: "Select Output Destination Folder",
      });
      if (result) {
        // Cancel any in-flight DB init for the previous destination.
        dbInitToken++;
        session.isInitializingDb = false;

        session.selectedOutput = Array.isArray(result) ? result[0] : result;
        appConfig.lastOutput = session.selectedOutput;
        appConfig.save();
      }
    } catch (err) {
      console.error("Directory error:", err);
    }
  }

  async function startBackup() {
    // Guard: If Snapchat export is split into mydata~<epoch>-N.zip parts and there's a gap,
    // don't start. The UI also disables the button, but keep this as a safety net.
    {
      const re = /(?:^|[\\/])mydata~(\d+)(?:-(\d+))?\.zip$/i;
      const byBase = new Map<string, Set<number>>();
      for (const p of session.selectedZips) {
        const m = p.match(re);
        if (!m) continue;
        const base = m[1];
        const part = m[2] ? Number(m[2]) : 1;
        if (!Number.isFinite(part) || part <= 0) continue;
        if (!byBase.has(base)) byBase.set(base, new Set());
        byBase.get(base)!.add(part);
      }
      for (const [base, set] of byBase.entries()) {
        const parts = Array.from(set).sort((a, b) => a - b);
        if (parts.length <= 1) continue;
        const min = parts[0];
        const max = parts[parts.length - 1];
        for (let i = min; i <= max; i++) {
          if (!set.has(i)) {
            console.error(`Missing ZIP part ${i} for mydata~${base}.`);
            session.isProcessing = false;
            session.statusMessage = `Missing ZIP part ${i} (mydata~${base}). Add it before starting.`;
            return;
          }
        }
      }
    }

    session.isProcessing = true;
    try {
      // Persist session so we can resume after restart.
      appConfig.lastSessionId = session.id;
      appConfig.save();
      await tauriService.startPipeline(
        session.id,
        appConfig.overwriteExisting,
        appConfig.maxConcurrency,
        session.selectedOutput,
      );
    } catch (err) {
      console.error("Pipeline error:", err);
      session.isProcessing = false;
    }
  }

  async function togglePause() {
    if (session.isPaused) {
      session.isProcessing = true;
      try {
        await tauriService.startPipeline(session.id, false, appConfig.maxConcurrency, session.selectedOutput);
        session.isPaused = false;
      } catch (err) {
        console.error("Resume error:", err);
      }
    } else {
      try {
        await tauriService.pausePipeline(session.id);
        session.isPaused = true;
        session.isProcessing = false;
      } catch (err) {
        console.error("Pause error:", err);
      }
    }
  }

  async function retryMemory(itemId: string) {
    try {
      await tauriService.retryItem(session.id, itemId);

      // Optimistic UI update: retry does not emit memory-updated immediately.
      const idx = session.memories.findIndex((m) => m.id === itemId);
      if (idx !== -1) {
        session.memories[idx] = {
          ...session.memories[idx],
          state: "Pending",
          errorMessage: undefined,
        };
      }

      session.isProcessing = true;
      session.isPaused = false;

      // Restart pipeline so the backend starts processing the reset item(s).
      await tauriService.startPipeline(
        session.id,
        false,
        appConfig.maxConcurrency,
        session.selectedOutput,
      );
    } catch (err) {
      console.error("Retry error:", err);
    }
  }

  async function cancelAndReset() {
    if (session.isProcessing || session.isPaused) {
      const confirmed = await ask(
        "This will cancel the current backup and start a new one. Continue?",
        { title: "Cancel backup", kind: "warning" },
      );
      if (!confirmed) return;
    }

    const oldId = session.id;
    // Stop backend work + clear DB/session data.
    try { await tauriService.pausePipeline(oldId); } catch {}
    try { await tauriService.cleanupDatabase(oldId); } catch {}
    try { await tauriService.closeSession(oldId); } catch {}

    appConfig.lastSessionId = null;
    appConfig.save();

    // New single session.
    session = new Session(crypto.randomUUID(), "Backup");
    hasAttemptedAppLoad = true; // don't auto-load into a "new" run
    await attachListeners(session.id);
  }


</script>

<div class="h-screen w-full flex flex-col bg-background text-foreground overflow-hidden font-sans">
  <Header />

  <main class="flex-1 overflow-hidden relative">
    <div class="absolute inset-0" in:fade={{ duration: 200 }}>
      <ProcessingView
        {session}
        onSelectOutput={handleSelectOutput}
        onStartBackup={startBackup}
        onTogglePause={togglePause}
        {...({ onRetryMemory: retryMemory } as any)}
        onAddZip={handleSelectZip}
        onDropZip={processZipPath}
        onRemoveZip={removeZip}
        onCancelAndReset={cancelAndReset}
      />
    </div>
  </main>
</div>
