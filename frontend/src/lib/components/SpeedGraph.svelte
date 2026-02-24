<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Chart, registerables, type ChartConfiguration } from "chart.js";

  Chart.register(...registerables);

  interface Props {
    data?: number[];
    labels?: string[];
    height?: number | string;
  }

  let { data = [], labels = [], height = "100%" }: Props = $props();

  let canvasEl: HTMLCanvasElement;
  let containerEl: HTMLDivElement;
  // $state so the data-sync $effect re-runs when chart is initialized (async)
  let chart = $state<Chart | null>(null);
  let resizeObserver: ResizeObserver | null = null;

  function createGradient(
    ctx: CanvasRenderingContext2D,
    chartArea: { bottom: number; top: number },
  ) {
    const gradient = ctx.createLinearGradient(
      0,
      chartArea.bottom,
      0,
      chartArea.top,
    );
    gradient.addColorStop(0, "rgba(0, 243, 255, 0)");
    gradient.addColorStop(0.2, "rgba(0, 243, 255, 0.05)");
    gradient.addColorStop(0.5, "rgba(0, 243, 255, 0.15)");
    gradient.addColorStop(0.8, "rgba(0, 243, 255, 0.3)");
    gradient.addColorStop(1, "rgba(0, 243, 255, 0.5)");
    return gradient;
  }

  onMount(() => {
    const initChart = async () => {
      await tick();
      const ctx = canvasEl.getContext("2d");
      if (!ctx) return;

      const config: ChartConfiguration = {
        type: "line",
        data: {
          labels: labels.length ? [...labels] : data.map((_, i) => ""),
          datasets: [
            {
              data: [...data],
              borderColor: "#00f3ff",
              borderWidth: 2.5,
              backgroundColor: (context: any) => {
                const chart = context.chart;
                const { ctx, chartArea } = chart;
                if (!chartArea) return "transparent";
                return createGradient(ctx, chartArea);
              },
              fill: true,
              tension: 0.6,
              cubicInterpolationMode: "monotone",
              pointRadius: (context: any) => {
                const index = context.dataIndex;
                const value = context.dataset.data[index] as number;
                const max = Math.max(...(context.dataset.data as number[]));
                return value === max && value > 0 ? 4 : 0;
              },
              pointBackgroundColor: "#fff",
              pointBorderColor: "#00f3ff",
              pointBorderWidth: 2,
              pointHoverRadius: 6,
              pointHoverBackgroundColor: "#00b8d4",
              pointHoverBorderColor: "#fff",
              pointHoverBorderWidth: 2,
              capBezierPoints: true,
            },
          ],
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          layout: {
            padding: {
              top: 15,
              bottom: 5,
              left: 0,
              right: 0,
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
              backgroundColor: "rgba(10, 15, 25, 0.9)",
              titleColor: "#00f3ff",
              titleFont: { size: 10, weight: "bold" },
              bodyColor: "#fff",
              bodyFont: { size: 12, family: "var(--font-mono)" },
              borderColor: "rgba(0, 243, 255, 0.2)",
              borderWidth: 1,
              padding: 10,
              displayColors: false,
              caretSize: 0,
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
              display: true,
              grid: {
                color: "rgba(255, 255, 255, 0.04)",
                drawTicks: false,
                lineWidth: 0.5,
              },
              ticks: {
                display: false,
              },
              border: {
                display: false,
                dash: [4, 4],
              },
              min: 0,
              suggestedMax: 10,
            },
          },
          animations: {
            y: {
              duration: 1000,
              easing: "easeInOutQuart",
            },
            tension: {
              duration: 1000,
              easing: "linear",
              from: 0.3,
              to: 0.6,
            },
          },
        },
      };

      chart = new Chart(ctx, config); // triggers the $effect below via $state reactivity

      // Watch for container resize — Chart.js needs explicit resize() when
      // the canvas parent starts at 0px and expands later (flex/grid layouts).
      resizeObserver = new ResizeObserver(() => {
        if (chart) {
          chart.resize();
        }
      });
      if (containerEl) resizeObserver.observe(containerEl);
    };

    initChart();

    return () => {
      resizeObserver?.disconnect();
      chart?.destroy();
    };
  });

  // Update chart when data changes OR when chart is first initialized.
  // chart is $state so this effect re-runs when the async initChart() sets it,
  // meaning any data that arrived before the chart was ready is applied immediately.
  $effect(() => {
    if (!chart || !data) return;

    chart.data.labels = labels.length ? [...labels] : data.map(() => "");
    chart.data.datasets[0].data = [...data];

    const maxVal = Math.max(...data, 1);
    if (chart.options.scales?.y) {
      chart.options.scales.y.suggestedMax = maxVal * 1.3;
    }

    // "none" = skip per-update animation so live ticks are instant, not jittery
    chart.update("none");
  });
</script>

<div
  class="speed-chart-container"
  style="height: {typeof height === 'number' ? height + 'px' : height};"
  bind:this={containerEl}
>
  <div class="chart-glow"></div>
  <canvas bind:this={canvasEl}></canvas>
</div>

<style>
  .speed-chart-container {
    width: 100%;
    height: 100%;
    position: relative;
    min-height: 100px;
    background: rgba(0, 0, 0, 0.1);
    border-radius: 8px;
    overflow: hidden;
  }

  .chart-glow {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 40px;
    background: linear-gradient(
      to bottom,
      rgba(0, 243, 255, 0.05),
      transparent
    );
    pointer-events: none;
  }

  canvas {
    filter: drop-shadow(0 0 10px rgba(0, 243, 255, 0.2));
  }
</style>
