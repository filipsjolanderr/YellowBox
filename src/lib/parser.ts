export interface SnapchatMemory {
    Date: string;
    "Media Type": "Image" | "Video";
    Location?: string | { latitude?: number; longitude?: number } | { Latitude?: number; Longitude?: number } | [number, number] | null;
    "Download Link": string;
    "Media Download Url": string;
}

export interface ParsedMemory {
    id: string;
    /** For split videos: segment IDs in playback order. Empty for single-segment. */
    segmentIds?: string[];
    downloadUrl: string;
    originalDate: string;
    location: string | null;
    state: string; // Dynamic mapping in Svelte vs Rust "Pending" | "Downloaded" | "Extracted" | "Combined" | "Completed" | "Failed"
    errorMessage?: string;
    extension?: string;
    hasOverlay: boolean;
    hasThumbnail: boolean;
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

        // Videos within 10 seconds of each other are segments of one long video (Snapchat 10-sec segments)
        const SEGMENT_GAP_SECONDS = 10; // solo <10s, combine >10s; overlay on both
        const videoEntries: Array<{ id: string; downloadUrl: string; location: string | null; dateStr: string; ts: number }> = [];
        const images: Array<{ id: string; downloadUrl: string; originalDate: string; location: string | null }> = [];

        for (const mem of rawMemories) {
            const link = mem["Download Link"] ?? mem["Media Download Url"];
            if (!link || typeof link !== "string") {
                console.warn("Skipping memory: missing Download Link", mem);
                continue;
            }
            let url: URL;
            try {
                url = new URL(link);
            } catch {
                console.warn("Skipping memory: invalid Download Link", link);
                continue;
            }
            const id = url.searchParams.get("mid") || url.searchParams.get("sid") || null;
            if (!id || id === "unknown-id") {
                console.warn("Skipping memory: no mid/sid in URL", link);
                continue;
            }
            const rawLocation = mem["Location"] ?? (mem as unknown as Record<string, unknown>)["location"];
            const location = normalizeLocation(rawLocation as LocationInput);
            const downloadUrl = mem["Media Download Url"] || mem["Download Link"] || link;

            if (mem["Media Type"] === "Video") {
                const dateStr = mem["Date"] ?? "";
                const ts = Date.parse(dateStr) || 0;
                videoEntries.push({ id, downloadUrl, location, dateStr, ts });
            } else {
                images.push({ id, downloadUrl, originalDate: mem["Date"] ?? "", location });
            }
        }

        // Sort by timestamp, then group consecutive videos within SEGMENT_GAP_SECONDS
        videoEntries.sort((a, b) => a.ts - b.ts);
        const videoGroups: Array<typeof videoEntries> = [];
        let currentGroup: typeof videoEntries = [];
        for (const v of videoEntries) {
            const prev = currentGroup[currentGroup.length - 1];
            if (currentGroup.length === 0 || (prev && v.ts - prev.ts <= SEGMENT_GAP_SECONDS * 1000)) {
                currentGroup.push(v);
            } else {
                videoGroups.push(currentGroup);
                currentGroup = [v];
            }
        }
        if (currentGroup.length > 0) videoGroups.push(currentGroup);

        // Build one memory per video (merged segments) or per image
        const result: ParsedMemory[] = [];

        for (const segments of videoGroups) {
            const segmentIds = segments.map((s) => s.id);
            const primaryId = segmentIds[0];
            const primary = segments[0];
            result.push({
                id: primaryId,
                segmentIds: segmentIds.length > 1 ? segmentIds : undefined,
                downloadUrl: primary.downloadUrl,
                originalDate: primary.dateStr,
                location: primary.location,
                state: "Pending" as const,
                hasOverlay: false,
                hasThumbnail: false,
                mediaType: "Video"
            });
        }

        // Include all images. Previously we skipped images with same date as video (overlay heuristic),
        // but that caused missing pictures. Better to include all; duplicates are preferable to loss.
        for (const img of images) {
            result.push({
                id: img.id,
                downloadUrl: img.downloadUrl,
                originalDate: img.originalDate,
                location: img.location,
                state: "Pending" as const,
                hasOverlay: false,
                hasThumbnail: false,
                mediaType: "Image"
            });
        }

        return result;

    } catch (e) {
        console.error("Failed to parse Snapchat Memories JSON:", e);
        return [];
    }
}
