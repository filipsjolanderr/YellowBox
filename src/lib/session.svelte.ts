import { tweened } from "svelte/motion";
import { cubicOut } from "svelte/easing";
import type { ParsedMemory } from "$lib/parser";

export class Session {
    id: string;
    name: string;

    selectedZips: string[] = $state([]);
    selectedOutput: string | null = $state(null);
    parsedItems: ParsedMemory[] = $state([]);
    memories: ParsedMemory[] = $state([]);
    isProcessing = $state(false);
    isPaused = $state(false);
    activeParsingTasks = $state(0);
    isParsing = $derived(this.activeParsingTasks > 0);
    isInitializingDb = $state(false);
    zipProgress: Record<string, number> = $state({});
    zipValidity: Record<string, "checking" | "valid" | "invalid"> = $state({});
    parsingProgress = tweened(0, { duration: 400, easing: cubicOut });
    statusMessage = $state("");
    hasAttemptedLoad = $state(false);

    pendingCount = $derived(
        this.memories.length > 0
            ? this.memories.filter((m) => m.state === "Pending").length
            : this.parsedItems.length
    );
    downloadedCount = $derived(this.memories.filter((m) => m.state === "Downloaded").length);
    extractedCount = $derived(this.memories.filter((m) => m.state === "Extracted").length);
    combinedCount = $derived(this.memories.filter((m) => m.state === "Combined").length);
    completedCount = $derived(this.memories.filter((m) => m.state === "Completed").length);
    failedCount = $derived(this.memories.filter((m) => m.state === "Failed").length);
    totalCount = $derived(this.memories.length > 0 ? this.memories.length : this.parsedItems.length);
    progressPercentage = $derived(this.totalCount > 0 ? (this.completedCount / this.totalCount) * 100 : 0);
    isAllProcessed = $derived(this.memories.length > 0 && this.completedCount + this.failedCount === this.memories.length);

    constructor(id: string, name: string) {
        this.id = id;
        this.name = name;
    }
}
