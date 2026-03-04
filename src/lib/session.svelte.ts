import { tweened } from "svelte/motion";
import { cubicOut } from "svelte/easing";
import type { ParsedMemory } from "$lib/parser";

export class Session {
    id: string;
    name: string;

    selectedZip: string | null = $state(null);
    selectedOutput: string | null = $state(null);
    parsedItems: ParsedMemory[] = $state([]);
    memories: ParsedMemory[] = $state([]);
    resolvedLocalPaths: Record<string, string> = $state({});
    isProcessing = $state(false);
    isPaused = $state(false);
    isParsingZip = $state(false);
    isInitializingDb = $state(false);
    parsingProgress = tweened(0, { duration: 400, easing: cubicOut });
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
