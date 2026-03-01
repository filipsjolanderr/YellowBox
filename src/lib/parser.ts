export interface SnapchatMemory {
    Date: string;
    "Media Type": "Image" | "Video";
    Location?: string | { latitude?: number; longitude?: number } | { Latitude?: number; Longitude?: number } | [number, number] | null;
    "Download Link": string;
    "Media Download Url": string;
}

export interface ParsedMemory {
    id: string;
    downloadUrl: string;
    originalDate: string;
    location: string | null;
    state: string; // Dynamic mapping in Svelte vs Rust "Pending" | "Downloaded" | "Extracted" | "Combined" | "Completed" | "Failed"
    errorMessage?: string;
    extension?: string;
    hasOverlay: boolean;
    mediaType: "Image" | "Video";
}

type LocationInput =
    | string
    | { latitude?: number; longitude?: number }
    | { Latitude?: number; Longitude?: number }
    | [number, number]
    | null
    | undefined;

/**
 * Normalizes location from various Snapchat export formats to "lat, lon" string or null.
 * Handles: "lat, lon" string, {latitude, longitude} object, [lat, lon] array, null/undefined.
 */
function normalizeLocation(raw: LocationInput): string | null {
    if (raw == null) return null;
    if (typeof raw === "string") {
        let trimmed = raw.trim();
        // Snapchat exports use "Latitude, Longitude: 57.686493, 11.977872" format
        const prefix = "Latitude, Longitude: ";
        if (trimmed.startsWith(prefix)) {
            trimmed = trimmed.slice(prefix.length).trim();
        }
        return trimmed.length > 0 ? trimmed : null;
    }
    if (Array.isArray(raw) && raw.length >= 2 && typeof raw[0] === "number" && typeof raw[1] === "number") {
        return `${raw[0]}, ${raw[1]}`;
    }
    if (typeof raw === "object") {
        const lat = (raw as { latitude?: number; Latitude?: number }).latitude ?? (raw as { latitude?: number; Latitude?: number }).Latitude;
        const lon = (raw as { longitude?: number; Longitude?: number }).longitude ?? (raw as { longitude?: number; Longitude?: number }).Longitude;
        if (typeof lat === "number" && typeof lon === "number") return `${lat}, ${lon}`;
    }
    return null;
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

            // Normalize location from various Snapchat export formats to "lat, lon" string
            const rawLocation = mem["Location"] ?? (mem as unknown as Record<string, unknown>)["location"];
            const location = normalizeLocation(rawLocation as LocationInput);

            return {
                id,
                downloadUrl: mem["Media Download Url"] || mem["Download Link"], // fallback if missing
                originalDate: mem["Date"],
                location,
                state: "Pending", // Initial state before checking processing status
                hasOverlay: false,
                mediaType: mem["Media Type"]
            };
        });

    } catch (e) {
        console.error("Failed to parse Snapchat Memories JSON:", e);
        return [];
    }
}
