import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ParsedMemory } from "$lib/parser";

export const tauriService = {
    async cleanupDatabase(sessionId: string): Promise<void> {
        await invoke("cleanup_database", { sessionId });
    },

    async clearAllData(): Promise<void> {
        await invoke("clear_all_data");
    },

    async checkZipStructure(sessionId: string, path: string): Promise<ParsedMemory[]> {
        return await invoke<ParsedMemory[]>("check_zip_structure", { sessionId, path });
    },

    async setExportPaths(sessionId: string, paths: string[]): Promise<void> {
        await invoke("set_export_paths", { sessionId, paths });
    },

    async initializeAndLoad(sessionId: string, outputPath: string, items: ParsedMemory[]): Promise<ParsedMemory[]> {
        return await invoke<ParsedMemory[]>("initialize_and_load", {
            sessionId,
            outputPath,
            items,
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
            session_id: "global", // fallback for stateless call
        } as any);
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

    async listenForPipelineStatus(sessionId: string, callback: (status: string) => void): Promise<UnlistenFn> {
        return await listen<{ message: string }>(`pipeline-status-${sessionId}`, (event) => {
            callback(event.payload.message);
        });
    },

    async listenForZipIndexingProgress(sessionId: string, callback: (payload: { path: string, progress: number }) => void): Promise<UnlistenFn> {
        return await listen<{ path: string, progress: number }>(`zip-indexing-progress-${sessionId}`, (event) => {
            callback(event.payload);
        });
    },

    getConvertedSrc(filePath: string): string {
        return convertFileSrc(filePath);
    }
};
