<script lang="ts">
    import * as Chart from "$lib/components/ui/chart/index.js";
    import { PieChart, Text } from "layerchart";
    import type { Session } from "$lib/session.svelte";

    let { session } = $props<{ session: Session }>();

    const chartConfig = {
        pending: { label: "Queued", color: "var(--color-muted-foreground)" },
        downloaded: { label: "Acquired", color: "var(--color-chart-1)" },
        extracted: { label: "Unpacked", color: "var(--color-chart-2)" },
        combined: { label: "Composited", color: "var(--color-chart-3)" },
        completed: { label: "Done", color: "var(--color-chart-4)" },
        failed: { label: "Error", color: "var(--color-destructive)" },
    } satisfies Chart.ChartConfig;

    let chartData = $derived([
        {
            state: "pending",
            count: session.pendingCount,
            color: chartConfig.pending.color,
        },
        {
            state: "downloaded",
            count: session.downloadedCount,
            color: chartConfig.downloaded.color,
        },
        {
            state: "extracted",
            count: session.extractedCount,
            color: chartConfig.extracted.color,
        },
        {
            state: "combined",
            count: session.combinedCount,
            color: chartConfig.combined.color,
        },
        {
            state: "completed",
            count: session.completedCount,
            color: chartConfig.completed.color,
        },
        {
            state: "failed",
            count: session.failedCount,
            color: chartConfig.failed.color,
        },
    ]);

    let totalActive = $derived(session.totalCount);
</script>

<div class="flex flex-col items-center justify-center p-2">
    <div class="h-[120px] w-[120px]">
        <Chart.Container
            config={chartConfig}
            class="mx-auto aspect-square h-full"
        >
            <PieChart
                data={chartData}
                key="state"
                value="count"
                c="color"
                innerRadius={0.7}
                cornerRadius={4}
                padding={{ top: 0, bottom: 0, left: 0, right: 0 }}
                props={{
                    tooltip: { root: { contained: "window", anchor: "right" } },
                }}
            >
                {#snippet aboveMarks()}
                    <Text
                        value={`${session.completedCount} / ${totalActive}`}
                        textAnchor="middle"
                        verticalAnchor="middle"
                        class="fill-foreground text-sm font-medium"
                    />
                {/snippet}
            </PieChart>
        </Chart.Container>
    </div>
</div>
