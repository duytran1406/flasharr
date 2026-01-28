<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    value: string | number;
    subtitle?: string;
    icon?: Snippet;
    color?: "blue" | "green" | "yellow" | "purple" | "cyan";
  }

  let { title, value, subtitle, icon, color = "blue" }: Props = $props();

  const colorClasses = {
    blue: "bg-blue-500/20 text-blue-400",
    green: "bg-green-500/20 text-green-400",
    yellow: "bg-yellow-500/20 text-yellow-400",
    purple: "bg-purple-500/20 text-purple-400",
    cyan: "bg-cyan-500/20 text-cyan-400",
  };
</script>

<div class="glass-card p-6 group hover:scale-[1.02] transition-transform">
  <div class="flex items-start justify-between">
    <div>
      <p class="text-slate-400 text-sm font-medium mb-1">{title}</p>
      <p class="text-3xl font-bold text-white tracking-tight">{value}</p>
      {#if subtitle}
        <p class="text-slate-500 text-sm mt-1">{subtitle}</p>
      {/if}
    </div>
    {#if icon}
      <div
        class="w-12 h-12 rounded-xl flex items-center justify-center {colorClasses[
          color
        ]} transition-transform group-hover:scale-110"
      >
        {@render icon()}
      </div>
    {/if}
  </div>
</div>
