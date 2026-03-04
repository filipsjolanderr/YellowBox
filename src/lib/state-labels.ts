/**
 * User-friendly labels for processing states.
 * API returns: Pending, Downloaded, Extracted, Combined, Completed, Failed, Paused
 */
export const STATE_LABELS: Record<string, string> = {
    Pending: "Queued",
    Downloaded: "Acquired",
    Extracted: "Unpacked",
    Combined: "Composited",
    Completed: "Done",
    Failed: "Error",
    Paused: "Paused",
};

export function getStateLabel(state: string): string {
    return STATE_LABELS[state] ?? state;
}
