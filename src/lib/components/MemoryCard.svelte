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
    } from "lucide-svelte";

    export let memory: ParsedMemory;
    export let selectedOutput: string | null;

    $: isDone = memory.state === "Completed" || memory.state === "Combined";
    $: isExtracted = memory.state === "Extracted";
    $: isZip =
        memory.extension === "zip" ||
        (!memory.extension &&
            memory.downloadUrl.toLowerCase().includes(".zip"));

    $: videoSrc = isDone
        ? getFinalSrc(memory)
        : isExtracted
          ? getLocalMainSrc(memory)
          : isZip
            ? ""
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

    function getFinalSrc(memory: ParsedMemory) {
        if (!selectedOutput) return "";
        const cleanDate = memory.originalDate
            .replace(" UTC", "")
            .replace(/:/g, "-")
            .replace(/ /g, "_");
        const ext = memory.extension || (isMaybeVideo(memory) ? "mp4" : "jpg");
        const filename = `${cleanDate}_${memory.id}.${ext}`;
        return tauriService.getConvertedSrc(`${selectedOutput}/${filename}`);
    }

    function getLocalMainSrc(memory: ParsedMemory) {
        if (!selectedOutput) return "";
        const ext = memory.extension || (isMaybeVideo(memory) ? "mp4" : "jpg");
        return tauriService.getConvertedSrc(
            `${selectedOutput}/${memory.id}-main.${ext}`,
        );
    }

    function getOverlaySrc(memory: ParsedMemory) {
        if (!selectedOutput) return "";
        return tauriService.getConvertedSrc(
            `${selectedOutput}/${memory.id}-overlay.png`,
        );
    }

    function isMaybeVideo(memory: ParsedMemory) {
        if (memory.mediaType === "Video") return true;
        if (memory.extension) {
            return ["mp4", "mov"].includes(memory.extension.toLowerCase());
        }
        const url = memory.downloadUrl.toLowerCase();
        return (
            url.includes(".mp4") ||
            url.includes(".mov") ||
            url.includes("video")
        );
    }
</script>

<Card
    class="group relative overflow-hidden transition-all hover:border-primary/50 hover:shadow-sm rounded-[4px] border border-border/50 p-0"
>
    <div class="aspect-[9/16] bg-black/5 relative overflow-hidden">
        {#if isMaybeVideo(memory)}
            <video
                use:lazyLoadVideo={videoSrc}
                class="absolute inset-0 h-full w-full object-cover transition-all duration-700 {isDone
                    ? 'opacity-100 grayscale-0 z-10'
                    : 'opacity-0 grayscale group-hover:grayscale-0'}"
                preload="none"
                muted
                playsinline
                onloadeddata={(e) => {
                    const vid = e.currentTarget as HTMLVideoElement;
                    if (!isDone) {
                        vid.classList.remove("opacity-0");
                        vid.classList.add(
                            "opacity-50",
                            "group-hover:opacity-100",
                        );
                    }
                    if (vid.previousElementSibling && !isDone) {
                        (
                            vid.previousElementSibling as HTMLElement
                        ).style.opacity = "0";
                    }
                }}
                onerror={(e) => {
                    const vid = e.currentTarget as HTMLVideoElement;
                    // Retry without timestamp or fallback to remote
                    if (vid.src.includes("#t=")) {
                        vid.src = vid.src.split("#")[0];
                        vid.load();
                    } else if (
                        (isDone || isExtracted) &&
                        vid.src !== memory.downloadUrl
                    ) {
                        console.warn(
                            `Local video failed for ${memory.id}, falling back to remote`,
                        );
                        vid.src = `${memory.downloadUrl}#t=0.5`;
                        vid.load();
                    } else {
                        vid.style.display = "none";
                    }
                }}
            ></video>
        {:else}
            <img
                src={isDone
                    ? getFinalSrc(memory)
                    : isExtracted
                      ? getLocalMainSrc(memory)
                      : isZip
                        ? ""
                        : memory.downloadUrl}
                alt="Memory"
                class="absolute inset-0 h-full w-full object-cover transition-all duration-700 {isDone
                    ? 'opacity-100 grayscale-0 z-10'
                    : 'opacity-0 grayscale group-hover:grayscale-0'}"
                loading="lazy"
                onload={(e) => {
                    const img = e.currentTarget as HTMLImageElement;
                    if (!isDone) {
                        img.classList.remove("opacity-0");
                        img.classList.add(
                            "opacity-50",
                            "group-hover:opacity-100",
                        );
                    }
                    if (img.previousElementSibling && !isDone) {
                        (
                            img.previousElementSibling as HTMLElement
                        ).style.opacity = "0";
                    }
                }}
                onerror={(e) => {
                    const img = e.currentTarget as HTMLImageElement;
                    if (
                        (isDone || isExtracted) &&
                        img.src !== memory.downloadUrl
                    ) {
                        console.warn(
                            `Local image failed for ${memory.id}, falling back to remote`,
                        );
                        img.src = memory.downloadUrl;
                    } else {
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
                class="absolute inset-x-0 bottom-0 top-1/2 z-30 flex flex-col justify-end bg-gradient-to-t from-red-900/90 to-red-500/10 pointer-events-auto overflow-hidden rounded-b-[4px]"
            >
                <div
                    class="text-white text-[9px] p-2 leading-tight font-medium max-h-full overflow-y-auto w-full"
                >
                    <span class="font-bold flex items-center mb-1"
                        ><CircleAlert class="h-3 w-3 mr-1" /> Error</span
                    >
                    {memory.errorMessage}
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
