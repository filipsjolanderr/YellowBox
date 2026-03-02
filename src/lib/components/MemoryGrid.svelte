<script lang="ts">
    import type { ParsedMemory } from "$lib/parser";
    import MemoryCard from "./MemoryCard.svelte";

    let { sessionId, memories, selectedOutput, resolvedLocalPaths = {}, isProcessing = false, isAllProcessed = false } = $props<{
        sessionId: string;
        memories: ParsedMemory[];
        selectedOutput: string | null;
        resolvedLocalPaths?: Record<string, string>;
        isProcessing?: boolean;
        isAllProcessed?: boolean;
    }>();
</script>

<div class="h-full overflow-y-auto p-4 content-start">
    <div
        class="grid grid-cols-4 sm:grid-cols-5 md:grid-cols-6 lg:grid-cols-8 xl:grid-cols-10 2xl:grid-cols-12 gap-2"
    >
        {#each memories as memory (memory.id)}
            <MemoryCard {sessionId} {memory} {selectedOutput} resolvedLocalPath={resolvedLocalPaths[memory.id]} {isProcessing} {isAllProcessed} />
        {/each}
    </div>
</div>
