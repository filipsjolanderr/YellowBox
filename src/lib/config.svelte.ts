export class AppConfig {
    concurrencyLimit = $state(8);
    overwriteExisting = $state(false);
    lastZip = $state<string | null>(null);
    lastOutput = $state<string | null>(null);

    constructor() {
        if (typeof window !== "undefined") {
            const stored = localStorage.getItem("yellowbox-config");
            if (stored) {
                try {
                    const parsed = JSON.parse(stored);
                    this.concurrencyLimit = parsed.concurrencyLimit ?? 8;
                    this.overwriteExisting = parsed.overwriteExisting ?? false;
                    this.lastZip = parsed.lastZip ?? null;
                    this.lastOutput = parsed.lastOutput ?? null;
                } catch (e) { }
            }
        }
    }

    save() {
        if (typeof window !== "undefined") {
            localStorage.setItem("yellowbox-config", JSON.stringify({
                concurrencyLimit: this.concurrencyLimit,
                overwriteExisting: this.overwriteExisting,
                lastZip: this.lastZip,
                lastOutput: this.lastOutput
            }));
        }
    }

    resetPrefs() {
        this.concurrencyLimit = 8;
        this.overwriteExisting = false;
        this.save();
    }
}

export const appConfig = new AppConfig();
