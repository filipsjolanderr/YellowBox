export class AppConfig {
    overwriteExisting = $state(false);
    maxConcurrency = $state<number | null>(null);
    lastZips = $state<string[]>([]);
    lastOutput = $state<string | null>(null);
    lastSessionId = $state<string | null>(null);

    constructor() {
        if (typeof window !== "undefined") {
            const stored = localStorage.getItem("yellowbox-config");
            if (stored) {
                try {
                    const parsed = JSON.parse(stored);
                    this.overwriteExisting = parsed.overwriteExisting ?? false;
                    this.maxConcurrency = parsed.maxConcurrency ?? null;
                    this.lastZips = parsed.lastZips ?? (parsed.lastZip ? [parsed.lastZip] : []);
                    this.lastOutput = parsed.lastOutput ?? null;
                    this.lastSessionId = parsed.lastSessionId ?? null;
                } catch (e) { }
            }
        }
    }

    save() {
        if (typeof window !== "undefined") {
            localStorage.setItem("yellowbox-config", JSON.stringify({
                overwriteExisting: this.overwriteExisting,
                maxConcurrency: this.maxConcurrency,
                lastZips: this.lastZips,
                lastOutput: this.lastOutput,
                lastSessionId: this.lastSessionId,
            }));
        }
    }

    resetPrefs() {
        this.overwriteExisting = false;
        this.maxConcurrency = null;
        this.save();
    }
}

export const appConfig = new AppConfig();
