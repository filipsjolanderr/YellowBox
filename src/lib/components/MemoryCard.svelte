<script lang="ts">
    import type { ParsedMemory } from "$lib/parser";
    import { tauriService } from "$lib/services/tauri";
    import { Card } from "$lib/components/ui/card";
    import { Badge } from "$lib/components/ui/badge";
    import {
        Video,
        Image as ImageIcon,
        CircleAlert,
        Archive,
        RefreshCw,
    } from "lucide-svelte";
    import { toast } from "svelte-sonner";

    export let sessionId: string;
    export let memory: ParsedMemory;
    export let selectedOutput: string | null;
    export let resolvedLocalPath: string | undefined = undefined;
    export let isProcessing = false;
    export let isAllProcessed = false;

    let localFallbackIndex = 0;
    let hasLoadedSuccessfully = false;
    const LOCAL_FALLBACKS: ((m: ParsedMemory) => string)[] = [
        getFinalSrcFallback,
        getLocalMainSrc,
        getLocalMainSrcFallback,
    ];

    $: isDone = memory.state === "Completed" || memory.state === "Combined";
    $: isExtracted = memory.state === "Extracted";
    $: isZip =
        memory.extension === "zip" ||
        (!memory.extension &&
            memory.downloadUrl.toLowerCase().includes(".zip"));

    $: thumbnailSrc =
        selectedOutput && (memory as any).hasThumbnail
            ? tauriService.getConvertedSrc(
                  selectedOutput.replace(/\\/g, "/") + `/.thumbs/${memory.id}.jpg`,
              )
            : "";

    // Prefer backend-resolved path when available; otherwise try guessed paths
    $: videoSrc = isZip
        ? ""
        : resolvedLocalPath
          ? tauriService.getConvertedSrc(resolvedLocalPath)
          : selectedOutput
            ? getFinalSrc(memory)
            : `${memory.downloadUrl}#t=0.5`;

    function lazyLoadVideo(node: HTMLVideoElement, src: string) {
        const observer = new IntersectionObserver(
            (entries) => {
                if (entries[0].isIntersecting && src) {
                    node.src = src;
                    node.load();
                    observer.unobserve(node);
                }
            },
            { rootMargin: "200px" },
        );

        if (src) observer.observe(node);

        return {
            update(newSrc: string) {
                if (newSrc !== src) {
                    src = newSrc;
                    // Only update immediately if already intersecting/showing
                    if (node.src) {
                        node.src = newSrc;
                        node.load();
                    }
                }
            },
            destroy() {
                observer.unobserve(node);
            },
        };
    }

    function getStateColor(state: string) {
        switch (state) {
            case "Pending":
                return "bg-secondary text-black";
            case "Downloaded":
                return "bg-blue-500 text-white";
            case "Extracted":
                return "bg-orange-400 text-white";
            case "Combined":
                return "bg-yellow-400 text-black";
            case "Completed":
                return "bg-green-500 text-white";
            case "Failed":
                return "bg-destructive text-destructive";
            default:
                return "bg-secondary";
        }
    }

    function toLocalPath(filename: string): string {
        if (!selectedOutput) return "";
        const base = selectedOutput.replace(/\\/g, "/");
        return `${base}/${filename}`;
    }

    function getFinalSrc(memory: ParsedMemory) {
        if (!selectedOutput) return "";
        const cleanDate = memory.originalDate
            .replace(" UTC", "")
            .replace(/:/g, "-")
            .replace(/ /g, "_");
        const ext = memory.extension || (isMaybeVideo(memory) ? "mp4" : "jpg");
        const filename = `${cleanDate}_${memory.id}.${ext}`;
        return tauriService.getConvertedSrc(toLocalPath(filename));
    }

    function getLocalMainSrc(memory: ParsedMemory) {
        if (!selectedOutput) return "";
        const ext = memory.extension || (isMaybeVideo(memory) ? "mp4" : "jpg");
        return tauriService.getConvertedSrc(toLocalPath(`${memory.id}-main.${ext}`));
    }

    function getLocalMainSrcFallback(memory: ParsedMemory) {
        if (!selectedOutput) return "";
        const ext = memory.extension || (isMaybeVideo(memory) ? "mp4" : "jpg");
        const cleanDate = memory.originalDate
            .replace(" UTC", "")
            .replace(/:/g, "-")
            .replace(/ /g, "_");
        return tauriService.getConvertedSrc(toLocalPath(`${cleanDate}_${memory.id}-main.${ext}`));
    }

    function getFinalSrcFallback(memory: ParsedMemory) {
        if (!selectedOutput) return "";
        const ext = memory.extension || (isMaybeVideo(memory) ? "mp4" : "jpg");
        return tauriService.getConvertedSrc(toLocalPath(`${memory.id}.${ext}`));
    }

    function getOverlaySrc(memory: ParsedMemory) {
        if (!selectedOutput) return "";
        return tauriService.getConvertedSrc(toLocalPath(`${memory.id}-overlay.png`));
    }

    function isMaybeVideo(memory: ParsedMemory) {
        if (memory.mediaType === "Video") return true;
        if (memory.extension) {
            const lowExt = memory.extension.toLowerCase();
            return ["mp4", "mov", "m4v", "mkv"].includes(lowExt);
        }
        const url = memory.downloadUrl.toLowerCase();
        return (
            url.includes(".mp4") ||
            url.includes(".mov") ||
            url.includes("video") ||
            url.includes("m4v")
        );
    }

    async function handleRetry(e: MouseEvent) {
        e.stopPropagation();
        try {
            await tauriService.retryItem(sessionId, memory.id);
            // After resetting state to Pending, give it a tiny delay then kick off pipeline processing
            setTimeout(() => {
                tauriService.startPipeline(sessionId, false).catch((err) => {
                    toast.error(`Auto-resume failed: ${err}`);
                });
            }, 300);
        } catch (err) {
            toast.error(`Retry failed: ${err}`);
        }
    }
</script>

<Card
    class="group relative overflow-hidden transition-all hover:border-primary/50 hover:shadow-sm rounded-[4px] border border-border/50 p-0"
>
    <div class="aspect-[9/16] bg-black/5 relative overflow-hidden">
        {#if thumbnailSrc}
            <img
                src={thumbnailSrc}
                alt="Preview"
                class="absolute inset-0 h-full w-full object-cover z-0"
            />
        {/if}

        {#if isMaybeVideo(memory)}
            <video
                use:lazyLoadVideo={videoSrc}
                poster={thumbnailSrc}
                class="absolute inset-0 h-full w-full object-cover transition-all duration-700 {(isAllProcessed && !isProcessing)
                    ? 'opacity-100 grayscale-0 z-10'
                    : 'opacity-0 grayscale group-hover:grayscale-0 z-10'}"
                preload="none"
                muted
                playsinline
                onloadeddata={(e) => {
                    hasLoadedSuccessfully = true;
                    const vid = e.currentTarget as HTMLVideoElement;
                    vid.style.display = "";
                    if (!isAllProcessed || isProcessing) {
                        vid.classList.remove("opacity-0");
                        vid.classList.add(
                            "opacity-50",
                            "group-hover:opacity-100",
                        );
                    }
                }}
                onerror={(e) => {
                    const vid = e.currentTarget as HTMLVideoElement;
                    // Retry without timestamp
                    if (vid.src.includes("#t=")) {
                        vid.src = vid.src.split("#")[0];
                        vid.load();
                        return;
                    }
                    const hasLocalSource = resolvedLocalPath || selectedOutput;
                    // Try next local path in cascade before remote
                    if (selectedOutput && localFallbackIndex < LOCAL_FALLBACKS.length) {
                        const fallback = LOCAL_FALLBACKS[localFallbackIndex](memory);
                        localFallbackIndex += 1;
                        if (fallback && vid.src !== fallback) {
                            vid.src = fallback;
                            vid.load();
                            return;
                        }
                    }
                    // Fall back to remote when local failed (resolvedLocalPath or selectedOutput)
                    if (hasLocalSource && vid.src !== memory.downloadUrl) {
                        console.warn(
                            `Local video failed for ${memory.id}, falling back to remote`,
                        );
                        vid.src = `${memory.downloadUrl}#t=0.5`;
                        vid.load();
                    } else if (!hasLoadedSuccessfully) {
                        vid.style.display = "none";
                    }
                }}
            ></video>
        {:else}
            <img
                src={isZip
                    ? ""
                    : resolvedLocalPath
                      ? tauriService.getConvertedSrc(resolvedLocalPath)
                      : selectedOutput
                        ? getFinalSrc(memory)
                        : memory.downloadUrl}
                alt="Memory"
                class="absolute inset-0 h-full w-full object-cover transition-all duration-700 {(isAllProcessed && !isProcessing)
                    ? 'opacity-100 grayscale-0 z-10'
                    : 'opacity-0 grayscale group-hover:grayscale-0 z-10'}"
                loading="lazy"
                onload={(e) => {
                    hasLoadedSuccessfully = true;
                    const img = e.currentTarget as HTMLImageElement;
                    img.style.display = "";
                    if (!isAllProcessed || isProcessing) {
                        img.classList.remove("opacity-0");
                        img.classList.add(
                            "opacity-50",
                            "group-hover:opacity-100",
                        );
                    }
                }}
                onerror={(e) => {
                    const img = e.currentTarget as HTMLImageElement;
                    const hasLocalSource = resolvedLocalPath || selectedOutput;
                    // Try next local path in cascade before remote
                    if (selectedOutput && localFallbackIndex < LOCAL_FALLBACKS.length) {
                        const fallback = LOCAL_FALLBACKS[localFallbackIndex](memory);
                        localFallbackIndex += 1;
                        if (fallback && img.src !== fallback) {
                            img.src = fallback;
                            return;
                        }
                    }
                    // Fall back to remote when local failed (resolvedLocalPath or selectedOutput)
                    if (hasLocalSource && img.src !== memory.downloadUrl) {
                        console.warn(
                            `Local image failed for ${memory.id}, falling back to remote`,
                        );
                        img.src = memory.downloadUrl;
                    } else if (!hasLoadedSuccessfully) {
                        img.style.display = "none";
                    }
                }}
            />
        {/if}

        {#if isZip && !isExtracted && !isDone}
            <div
                class="absolute inset-0 flex flex-col items-center justify-center bg-muted/20 z-0"
            >
                <Archive
                    class="h-6 w-6 text-muted-foreground/30 animate-pulse"
                />
                <span
                    class="mt-2 text-[8px] uppercase tracking-widest text-muted-foreground/40 font-bold"
                    >Zip Bundle</span
                >
            </div>
        {/if}

        {#if isExtracted && memory.hasOverlay}
            <img
                src={getOverlaySrc(memory)}
                alt="Overlay"
                class="absolute inset-0 h-full w-full object-contain z-20 pointer-events-none"
                onerror={(e) => {
                    (e.currentTarget as HTMLImageElement).style.display =
                        "none";
                }}
            />
        {/if}

        {#if memory.errorMessage}
            <div
                class="absolute inset-x-0 bottom-0 top-1/2 z-30 flex flex-col justify-end bg-gradient-to-t from-red-900/95 to-red-500/20 pointer-events-auto overflow-hidden rounded-b-[4px]"
            >
                <div
                    class="text-white text-[9px] p-2 leading-tight font-medium max-h-full overflow-y-auto w-full flex flex-col justify-end gap-2 h-full"
                >
                    <button
                        onclick={handleRetry}
                        class="mb-1 flex items-center justify-center gap-1 rounded bg-red-600 hover:bg-red-500 p-1.5 text-white transition-colors cursor-pointer shadow-sm font-bold uppercase tracking-tighter"
                    >
                        <RefreshCw class="h-3 w-3" /> Retry Item
                    </button>
                    <div class="opacity-90">
                        <span
                            class="font-bold flex items-center mb-0.5 text-red-100"
                            ><CircleAlert class="h-3 w-3 mr-1" /> Error</span
                        >
                        {memory.errorMessage}
                    </div>
                </div>
            </div>
        {/if}

        <Badge
            variant="secondary"
            class="absolute top-1.5 left-1.5 z-20 h-5 w-5 p-0 flex items-center justify-center bg-background/60 backdrop-blur-md shadow-sm border-0"
        >
            {#if isMaybeVideo(memory)}
                <Video class="h-3 w-3 text-foreground/80" />
            {:else}
                <ImageIcon class="h-3 w-3 text-foreground/80" />
            {/if}
        </Badge>

        <div
            class="absolute top-1.5 right-1.5 z-20 flex flex-col gap-1 items-end pointer-events-none"
        >
            <Badge
                variant="secondary"
                class="text-[8px] px-1.5 py-0 h-4 border-0 shadow-sm rounded-[3px] font-medium leading-none bg-background/80 backdrop-blur-md text-foreground/80"
            >
                {new Date(memory.originalDate).toLocaleDateString(undefined, {
                    month: "short",
                    day: "numeric",
                    year: "2-digit",
                })}
            </Badge>
        </div>

        <div
            class="absolute bottom-0 left-0 right-0 p-1.5 pb-1.5 flex justify-end items-end bg-gradient-to-t from-black/80 via-black/30 to-transparent z-20 pointer-events-none"
        >
            <div class="flex flex-col gap-1 items-end">
                <Badge
                    class="text-[9px] px-1.5 py-0 h-4 {getStateColor(
                        memory.state,
                    )} border-0 shadow-sm rounded-[3px] font-medium leading-none text-white"
                >
                    {memory.state === "Pending" ? "Ready" : memory.state}
                </Badge>
            </div>
        </div>
    </div>
</Card>
