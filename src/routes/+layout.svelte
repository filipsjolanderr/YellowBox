<script lang="ts">
    import "../app.css";
    import { Toaster } from "svelte-sonner";
    import { ModeWatcher } from "mode-watcher";

    let { children } = $props();
</script>

<ModeWatcher />
<Toaster position="bottom-right" richColors />
{@render children()}
