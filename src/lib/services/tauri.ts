import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ParsedMemory } from "$lib/parser";

export const tauriService = {
    async cleanupDatabase(): Promise<void> {
        await invoke("cleanup_database");
    },

    async checkZipStructure(path: string): Promise<string> {
        return await invoke<string>("check_zip_structure", { path });
    },

    async initializeAndLoad(outputPath: string, items: ParsedMemory[]): Promise<ParsedMemory[]> {
        return await invoke<ParsedMemory[]>("initialize_and_load", {
            outputPath,
            items,
        });
    },

    async startPipeline(concurrencyLimit: number, overwriteExisting: boolean): Promise<void> {
        await invoke("start_pipeline", { concurrencyLimit, overwriteExisting });
    },

    async resetApplication(): Promise<void> {
        await invoke("reset_application");
    },

    async listenForMemoryUpdates(callback: (memory: ParsedMemory) => void): Promise<UnlistenFn> {
        return await listen<ParsedMemory>("memory-updated", (event) => {
            callback(event.payload);
        });
    },

    getConvertedSrc(filePath: string): string {
        return convertFileSrc(filePath);
    }
};
