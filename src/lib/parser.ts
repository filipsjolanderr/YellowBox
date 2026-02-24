export interface SnapchatMemory {
    Date: string;
    "Media Type": "Image" | "Video";
    Location: string;
    "Download Link": string;
    "Media Download Url": string;
}

export interface ParsedMemory {
    id: string;
    downloadUrl: string;
    originalDate: string;
    location: string;
    state: string; // Dynamic mapping in Svelte vs Rust "Pending" | "Downloaded" | "Extracted" | "Combined" | "Completed" | "Failed"
    errorMessage?: string;
    extension?: string;
}

/**
 * Parses the raw json/memories_history.json text content and returns a list of formatted memories
 * ready for the backend SQLite tracker or UI rendering.
 */
export function parseMemoriesJson(jsonContent: string): ParsedMemory[] {
    try {
        const payload = JSON.parse(jsonContent);
        if (!payload["Saved Media"]) {
            throw new Error("Invalid Format: Could not find 'Saved Media' array in JSON.");
        }

        const rawMemories: SnapchatMemory[] = payload["Saved Media"];

        return rawMemories.map(mem => {
            // Extract a unique identifier from the download link (mid or sid)
            const url = new URL(mem["Download Link"]);
            const id = url.searchParams.get("mid") || url.searchParams.get("sid") || "unknown-id";

            return {
                id,
                downloadUrl: mem["Media Download Url"] || mem["Download Link"], // fallback if missing
                originalDate: mem["Date"],
                location: mem["Location"],
                state: "Pending" // Initial state before checking processing status
            };
        });

    } catch (e) {
        console.error("Failed to parse Snapchat Memories JSON:", e);
        return [];
    }
}
