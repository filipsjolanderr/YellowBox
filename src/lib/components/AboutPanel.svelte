<script lang="ts">
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
  } from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import { Separator } from "$lib/components/ui/separator";
  import { Info, ShieldCheck, Cpu, FolderKey } from "lucide-svelte";
  import * as Tooltip from "$lib/components/ui/tooltip";

  const version = import.meta.env.VITE_APP_VERSION ?? "0.0.0";
</script>

<Dialog>
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props: tooltipProps })}
        <div {...tooltipProps} class="inline-block">
          <DialogTrigger>
            {#snippet child({ props: dialogProps })}
              <Button
                {...dialogProps}
                variant="ghost"
                size="icon"
                class="h-8 w-8 text-muted-foreground hover:text-foreground"
              >
                <Info class="h-4 w-4" />
                <span class="sr-only">About</span>
              </Button>
            {/snippet}
          </DialogTrigger>
        </div>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content sideOffset={4}>
      <p>About YellowBox</p>
    </Tooltip.Content>
  </Tooltip.Root>
  <DialogContent class="sm:max-w-[400px]">
    <DialogHeader class="space-y-1.5">
      <div class="flex items-center gap-2">
        <DialogTitle>About YellowBox</DialogTitle>
        <Badge variant="secondary" class="font-normal">v{version}</Badge>
      </div>
      <DialogDescription>
        Securely parses and reconstructs your exported Snapchat Memories.
      </DialogDescription>
    </DialogHeader>

    <div class="space-y-4 pt-2">
      <div class="space-y-3">
        <div class="flex gap-3">
          <div
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-primary/10"
          >
            <ShieldCheck class="h-4 w-4 text-primary" />
          </div>
          <div class="min-w-0 space-y-0.5">
            <p class="text-sm font-medium">Privacy-first</p>
            <p class="text-xs text-muted-foreground leading-relaxed">
              Your data stays on your machine. No uploads, no accounts.
            </p>
          </div>
        </div>
        <div class="flex gap-3">
          <div
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-primary/10"
          >
            <Cpu class="h-4 w-4 text-primary" />
          </div>
          <div class="min-w-0 space-y-0.5">
            <p class="text-sm font-medium">Built for speed</p>
            <p class="text-xs text-muted-foreground leading-relaxed">
              Parallel processing with configurable limits.
            </p>
          </div>
        </div>
        <div class="flex gap-3">
          <div
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-primary/10"
          >
            <FolderKey class="h-4 w-4 text-primary" />
          </div>
          <div class="min-w-0 space-y-0.5">
            <p class="text-sm font-medium">Local output</p>
            <p class="text-xs text-muted-foreground leading-relaxed">
              Results are saved to the folder you choose, ready to browse.
            </p>
          </div>
        </div>
      </div>

      <Separator />

      <div class="flex flex-wrap gap-1.5">
        <Badge variant="outline">Rust</Badge>
        <Badge variant="outline">Tauri</Badge>
        <Badge variant="outline">Svelte 5</Badge>
      </div>

      <Separator />

      <p class="text-xs text-muted-foreground">
        Made by <span class="font-medium text-foreground">Filip Sjölander</span>
        ·
        <a
          class="text-primary underline-offset-4 hover:underline"
          href="https://sjolander.dev"
          target="_blank"
          rel="noreferrer"
        >
          sjolander.dev
        </a>
      </p>
    </div>
  </DialogContent>
</Dialog>
