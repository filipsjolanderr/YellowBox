import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ParsedMemory } from "$lib/parser";

export const tauriService = {
    async cleanupDatabase(sessionId: string): Promise<void> {
        await invoke("cleanup_database", { sessionId });
    },

    async checkZipStructure(sessionId: string, path: string): Promise<string> {
        return await invoke<string>("check_zip_structure", { sessionId, path });
    },

    async initializeAndLoad(sessionId: string, outputPath: string, items: ParsedMemory[]): Promise<ParsedMemory[]> {
        return await invoke<ParsedMemory[]>("initialize_and_load", {
            sessionId,
            outputPath,
            items,
        });
    },

    async clearPreviewTemp(sessionId: string): Promise<void> {
        await invoke("clear_preview_temp", { sessionId });
    },

    async extractPreviewMedia(sessionId: string, zipPath: string, memoryIds: string[]): Promise<string> {
        return await invoke<string>("extract_preview_media", {
            sessionId,
            zipPath,
            memoryIds,
        });
    },

    async resolveLocalMediaPaths(sessionId: string, memoryIds: string[]): Promise<Record<string, string>> {
        const result = await invoke<Record<string, string>>("resolve_local_media_paths", {
            sessionId,
            memoryIds,
        });
        return result ?? {};
    },

    async startPipeline(sessionId: string, overwriteExisting: boolean, maxConcurrency?: number | null, outputPath?: string | null): Promise<void> {
        await invoke("start_pipeline", {
            sessionId,
            overwriteExisting,
            maxConcurrency: maxConcurrency ?? null,
            outputPath: outputPath ?? null,
        });
    },

    async pausePipeline(sessionId: string): Promise<void> {
        await invoke("pause_pipeline", { sessionId });
    },

    async checkOverlayExists(outputDir: string, memoryId: string, cleanDate: string): Promise<boolean> {
        return await invoke<boolean>("check_overlay_exists", {
            outputDir,
            memoryId,
            cleanDate,
        });
    },

    async retryItem(sessionId: string, itemId: string): Promise<void> {
        await invoke("retry_item", { sessionId, itemId });
    },

    async closeSession(sessionId: string): Promise<void> {
        await invoke("reset_application", { sessionId });
    },

    async listenForMemoryUpdates(sessionId: string, callback: (memory: ParsedMemory) => void): Promise<UnlistenFn> {
        return await listen<ParsedMemory>(`memory-updated-${sessionId}`, (event) => {
            callback(event.payload);
        });
    },

    getConvertedSrc(filePath: string): string {
        return convertFileSrc(filePath);
    }
};
