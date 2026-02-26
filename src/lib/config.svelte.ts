export class AppConfig {
    overwriteExisting = $state(false);
    lastZip = $state<string | null>(null);
    lastOutput = $state<string | null>(null);

    constructor() {
        if (typeof window !== "undefined") {
            const stored = localStorage.getItem("yellowbox-config");
            if (stored) {
                try {
                    const parsed = JSON.parse(stored);
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
                overwriteExisting: this.overwriteExisting,
                lastZip: this.lastZip,
                lastOutput: this.lastOutput
            }));
        }
    }

    resetPrefs() {
        this.overwriteExisting = false;
        this.save();
    }
}

export const appConfig = new AppConfig();
