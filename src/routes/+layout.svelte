<script lang="ts">
    import "../app.css";
    import { ModeWatcher } from "mode-watcher";

    import * as Tooltip from "$lib/components/ui/tooltip";

    let { children } = $props();
</script>

<ModeWatcher />
<Tooltip.Provider delayDuration={200}>
    {@render children()}
</Tooltip.Provider>
