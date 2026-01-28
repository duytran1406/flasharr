<script lang="ts">
  interface Props {
    speed: number;
    unit?: string;
  }

  let { speed, unit = "MB/s" }: Props = $props();
</script>

<div class="text-center py-8">
  <div class="relative inline-block">
    <!-- Glow effect -->
    <div class="absolute inset-0 blur-3xl bg-blue-500/30 scale-150"></div>

    <!-- Speed value -->
    <div class="relative">
      <span class="text-7xl font-bold text-white tracking-tight glow-text">
        {speed.toFixed(1)}
      </span>
      <span class="text-3xl font-medium text-blue-400 ml-2">{unit}</span>
    </div>
  </div>

  <p class="text-slate-400 mt-3 text-lg">Total Download Speed</p>
</div>
