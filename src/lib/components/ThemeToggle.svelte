<script lang="ts">
    import { Moon, Sun } from "lucide-svelte";
    import { toggleMode } from "mode-watcher";
    import { Button } from "$lib/components/ui/button";
</script>

<Button
    variant="ghost"
    size="icon"
    class="h-8 w-8 text-muted-foreground hover:text-foreground"
    onclick={toggleMode}
>
    <Sun
        class="h-4 w-4 rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0"
    />
    <Moon
        class="absolute h-4 w-4 rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100"
    />
    <span class="sr-only">Toggle theme</span>
</Button>
