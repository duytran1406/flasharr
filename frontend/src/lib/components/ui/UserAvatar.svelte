<script lang="ts">
  interface Props {
    src?: string;
    size?: number;
    status?: "VIP" | "Premium" | "GUEST" | "ACTIVE" | "INACTIVE";
    glow?: boolean;
  }

  let {
    src = "/images/logo_fshare.png",
    size = 48,
    status = "GUEST",
    glow = true,
  }: Props = $props();

  const isVIP = $derived(status === "VIP");
  const isPremium = $derived(status === "Premium");
</script>

<div
  class="avatar-container"
  style="--size: {size}px;"
  class:vip={isVIP}
  class:premium={isPremium}
  class:guest={!isVIP && !isPremium}
  class:has-glow={glow}
>
  <div class="avatar-ring">
    <div class="avatar-inner">
      <div class="avatar-bg">
        <img {src} alt="Account Avatar" />
      </div>
    </div>
  </div>

  <!-- Status Indicator Dot -->
  <div
    class="status-dot"
    class:active={isVIP || isPremium}
    title={status}
  ></div>
</div>

<style>
  .avatar-container {
    position: relative;
    width: var(--size);
    height: var(--size);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .avatar-ring {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.3s ease;
    padding: 2px; /* Thinner padding for the ring border effect */
  }

  /* VIP Status Ring */
  .vip .avatar-ring {
    background: linear-gradient(135deg, #00f3ff 0%, #4facfe 100%);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.3);
  }

  /* Premium Status Ring */
  .premium .avatar-ring {
    background: linear-gradient(135deg, #f59e0b 0%, #fbbf24 100%);
    box-shadow: 0 0 15px rgba(245, 158, 11, 0.2);
  }

  /* Guest Status Ring */
  .guest .avatar-ring {
    background: rgba(255, 255, 255, 0.1);
  }

  .avatar-inner {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    position: relative;
    background: #0f172a;
    padding: 2px; /* Gap between ring and logo */
    overflow: hidden;
  }

  .avatar-bg {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #1e293b;
  }

  .avatar-bg img {
    width: 100%;
    height: 100%;
    object-fit: cover; /* Fill the circle entirely */
    border-radius: 50%;
  }

  /* Glow Effect */
  .has-glow.vip .avatar-ring {
    animation: pulse-vip 2s infinite;
  }

  @keyframes pulse-vip {
    0%,
    100% {
      box-shadow: 0 0 20px rgba(0, 243, 255, 0.3);
    }
    50% {
      box-shadow: 0 0 30px rgba(0, 243, 255, 0.5);
    }
  }

  /* Status Dot */
  .status-dot {
    position: absolute;
    bottom: 2px;
    right: 2px;
    width: 22%;
    height: 22%;
    min-width: 10px;
    min-height: 10px;
    background: #475569;
    border: 2px solid #0f172a;
    border-radius: 50%;
    z-index: 2;
  }

  .status-dot.active {
    background: #22c55e;
    box-shadow: 0 0 8px rgba(34, 197, 94, 0.4);
  }

  /* Hover Effect */
  .avatar-container:hover .avatar-ring {
    transform: scale(1.05);
  }
</style>
