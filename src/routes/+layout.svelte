<script lang="ts">
    import "../app.css";
    import { Toaster } from "svelte-sonner";
    import { ModeWatcher } from "mode-watcher";

    import * as Tooltip from "$lib/components/ui/tooltip";

    let { children } = $props();
</script>

<ModeWatcher />
<Toaster position="bottom-right" richColors />
<Tooltip.Provider delayDuration={200}>
    {@render children()}
</Tooltip.Provider>
