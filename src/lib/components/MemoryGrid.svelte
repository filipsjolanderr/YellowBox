<script lang="ts">
    import type { ParsedMemory } from "$lib/parser";
    import MemoryCard from "./MemoryCard.svelte";

    let { memories, selectedOutput } = $props<{
        memories: ParsedMemory[];
        selectedOutput: string | null;
    }>();
</script>

<div class="h-full overflow-y-auto p-4 content-start">
    <div
        class="grid grid-cols-4 sm:grid-cols-5 md:grid-cols-6 lg:grid-cols-8 xl:grid-cols-10 2xl:grid-cols-12 gap-2"
    >
        {#each memories as memory (memory.id)}
            <MemoryCard {memory} {selectedOutput} />
        {/each}
    </div>
</div>
