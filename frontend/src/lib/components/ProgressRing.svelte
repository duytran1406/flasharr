<script lang="ts">
  interface Props {
    progress: number;
    size?: number;
    strokeWidth?: number;
    color?: string;
    bgColor?: string;
  }

  let {
    progress,
    size = 60,
    strokeWidth = 6,
    color = "#3b82f6",
    bgColor = "rgba(255,255,255,0.1)",
  }: Props = $props();

  const radius = (size - strokeWidth) / 2;
  const circumference = radius * 2 * Math.PI;
  const offset = circumference - (progress / 100) * circumference;
</script>

<div
  class="relative inline-flex items-center justify-center"
  style="width: {size}px; height: {size}px;"
>
  <svg class="progress-ring" width={size} height={size}>
    <!-- Background circle -->
    <circle
      cx={size / 2}
      cy={size / 2}
      r={radius}
      fill="transparent"
      stroke={bgColor}
      stroke-width={strokeWidth}
    />
    <!-- Progress circle -->
    <circle
      class="progress-ring-circle"
      cx={size / 2}
      cy={size / 2}
      r={radius}
      fill="transparent"
      stroke={color}
      stroke-width={strokeWidth}
      stroke-linecap="round"
      stroke-dasharray={circumference}
      stroke-dashoffset={offset}
    />
  </svg>
  <div class="absolute inset-0 flex items-center justify-center">
    <span class="text-white font-bold text-sm">{Math.round(progress)}%</span>
  </div>
</div>
