<script lang="ts">
    import type { ParsedMemory } from "$lib/parser";
    import MemoryCard from "./MemoryCard.svelte";

    import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";

    let {
        sessionId,
        memories,
        selectedOutput,
        isProcessing = false,
        isAllProcessed = false,
    } = $props<{
        sessionId: string;
        memories: ParsedMemory[];
        selectedOutput: string | null;
        isProcessing?: boolean;
        isAllProcessed?: boolean;
    }>();

    const sortedMemories = $derived(
        [...memories].sort((a, b) => {
            const ta = Date.parse(a.originalDate) || 0;
            const tb = Date.parse(b.originalDate) || 0;
            return tb - ta; // newest first
        }),
    );
</script>

<ScrollArea class="h-full w-full">
    <div class="p-4">
        <div
            class="grid gap-2 content-start w-full"
            style="grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));"
        >
            {#each sortedMemories as memory (memory.id)}
                <MemoryCard
                    {sessionId}
                    {memory}
                    {selectedOutput}
                />
            {/each}
        </div>
    </div>
</ScrollArea>
