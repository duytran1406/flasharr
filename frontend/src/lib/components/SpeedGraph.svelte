<script lang="ts">
  import { onMount } from "svelte";
  import { Chart, registerables } from "chart.js";

  Chart.register(...registerables);

  interface Props {
    data?: number[];
    labels?: string[];
    height?: number | string;
  }

  let { data = [], labels = [], height = "100%" }: Props = $props();

  let canvasEl: HTMLCanvasElement;
  let chart: Chart;

  onMount(() => {
    const ctx = canvasEl.getContext("2d");
    if (!ctx) return;

    // Create gradient
    const gradientHeight =
      typeof height === "number" ? height : canvasEl.clientHeight;
    const gradient = ctx.createLinearGradient(0, 0, 0, gradientHeight || 200);
    gradient.addColorStop(0, "rgba(0, 243, 255, 0.15)");
    gradient.addColorStop(1, "rgba(0, 243, 255, 0)");

    chart = new Chart(ctx, {
      type: "line",
      data: {
        labels: labels.length ? [...labels] : data.map((_, i) => ""),
        datasets: [
          {
            data: [...data],
            borderColor: "#00f3ff",
            borderWidth: 2,
            backgroundColor: gradient,
            fill: true,
            tension: 0.4,
            pointRadius: 0,
            pointHitRadius: 10,
            pointHoverRadius: 4,
            pointHoverBackgroundColor: "#00f3ff",
            pointHoverBorderColor: "#fff",
            pointHoverBorderWidth: 2,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        layout: {
          padding: {
            top: 5,
            bottom: 5,
          },
        },
        interaction: {
          intersect: false,
          mode: "index",
        },
        plugins: {
          legend: { display: false },
          tooltip: {
            enabled: true,
            backgroundColor: "rgba(10, 15, 25, 0.95)",
            titleColor: "#00f3ff",
            titleFont: { size: 10, weight: "bold" },
            bodyColor: "#fff",
            bodyFont: { size: 12, family: "var(--font-mono)" },
            borderColor: "rgba(0, 243, 255, 0.2)",
            borderWidth: 1,
            padding: 10,
            displayColors: false,
            callbacks: {
              label: (context) => {
                const val = context.parsed.y;
                return val !== null ? `${val.toFixed(2)} MB/s` : "0.00 MB/s";
              },
            },
          },
        },
        scales: {
          x: {
            display: false,
          },
          y: {
            display: false,
            min: -0.1, // Offset slightly so 0 line is visible
            suggestedMax: 1, // Minimum max so it's not a single pixel line
          },
        },
      },
    });

    return () => {
      chart?.destroy();
    };
  });

  // Update chart when data changes
  $effect(() => {
    if (chart && data) {
      chart.data.labels = labels.length ? [...labels] : data.map((_, i) => "");
      chart.data.datasets[0].data = [...data];

      // Dynamic y-axis scaling based on max value in data
      const maxVal = Math.max(...data, 0.1);
      if (chart.options.scales?.y) {
        chart.options.scales.y.suggestedMax = maxVal * 1.5;
      }

      chart.update("none");
    }
  });
</script>

<div
  class="speed-chart-container"
  style="height: {typeof height === 'number' ? height + 'px' : height};"
>
  <canvas bind:this={canvasEl}></canvas>
</div>

<style>
  .speed-chart-container {
    width: 100%;
    height: 100%;
    position: relative;
    min-height: 100px;
  }
</style>
