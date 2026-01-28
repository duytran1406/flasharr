<script lang="ts">
  import UserAvatar from "./UserAvatar.svelte";

  interface Props {
    email: string;
    rank: string;
    expiry: string;
    quotaUsed: string;
    quotaTotal: string;
    quotaPercent: number;
    compact?: boolean;
    onRefresh?: () => void;
    onLogout?: () => void;
  }

  let {
    email,
    rank,
    expiry,
    quotaUsed,
    quotaTotal,
    quotaPercent,
    compact = false,
    onRefresh,
    onLogout,
  }: Props = $props();

  const isVIP = $derived(rank.includes("VIP"));
  const isPremium = $derived(rank.includes("Premium") && !isVIP);
</script>

<div class="identity-card" class:compact>
  <div class="identity-header">
    <UserAvatar
      status={isVIP ? "VIP" : isPremium ? "Premium" : "GUEST"}
      size={compact ? 44 : 56}
    />
    <div class="identity-main">
      <div class="email-row">
        <span class="email">{email}</span>
      </div>
      <div class="status-row">
        <span
          class="status-pill"
          class:vip={isVIP}
          class:premium={isPremium}
          class:guest={!isVIP && !isPremium}
        >
          {isVIP ? "VIP" : isPremium ? "PREMIUM" : "FREE"}
        </span>
        <span class="expiry-text">· {expiry}</span>
      </div>
    </div>

    {#if onRefresh || onLogout}
      <div class="identity-actions">
        {#if onRefresh}
          <button
            class="action-btn refresh"
            onclick={onRefresh}
            title="Refresh Account"
          >
            <span class="material-icons">refresh</span>
          </button>
        {/if}
        {#if onLogout}
          <button
            class="action-btn logout"
            onclick={onLogout}
            title="Logout Account"
          >
            <span class="material-icons">logout</span>
          </button>
        {/if}
      </div>
    {/if}
  </div>

  <div class="quota-system">
    <div class="quota-meta">
      <span class="quota-label">Daily Quota</span>
      <span class="quota-val">{quotaPercent}%</span>
    </div>
    <div class="quota-track">
      <div class="quota-fill" style="width: {quotaPercent}%;">
        <div class="shimmer"></div>
      </div>
    </div>
    <div class="quota-footer">
      <span class="used">{quotaUsed}</span>
      <span class="separator">/</span>
      <span class="total">{quotaTotal}</span>
    </div>
  </div>
</div>

<style>
  .identity-card {
    background: rgba(15, 23, 42, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 1.5rem;
    backdrop-filter: blur(10px);
    position: relative;
    overflow: hidden;
  }

  .identity-header {
    display: flex;
    gap: 1rem;
    align-items: center;
    margin-bottom: 1.5rem;
  }

  .identity-main {
    flex: 1;
    min-width: 0;
  }

  .email-row {
    margin-bottom: 0.5rem;
  }

  .email {
    font-size: 0.95rem;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  /* Status Pill Badge */
  .status-pill {
    font-size: 0.65rem;
    font-weight: 800;
    padding: 4px 10px;
    border-radius: 20px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .status-pill.vip {
    background: linear-gradient(135deg, #00f3ff 0%, #4facfe 100%);
    color: #000;
    box-shadow: 0 2px 10px rgba(0, 243, 255, 0.3);
  }

  .status-pill.premium {
    background: linear-gradient(135deg, #f59e0b 0%, #fbbf24 100%);
    color: #000;
  }

  .status-pill.guest {
    background: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.6);
  }

  .expiry-text {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.5);
  }

  /* Identity Actions */
  .identity-actions {
    display: flex;
    gap: 0.5rem;
    align-self: flex-start;
  }

  .action-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.6);
    width: 32px;
    height: 32px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .action-btn .material-icons {
    font-size: 1.1rem;
  }

  .action-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.2);
    transform: translateY(-2px);
  }

  .action-btn.refresh:hover {
    color: var(--color-primary, #00f3ff);
    box-shadow: 0 4px 12px rgba(0, 243, 255, 0.2);
  }

  .action-btn.logout:hover {
    color: #ef4444;
    box-shadow: 0 4px 12px rgba(239, 68, 68, 0.2);
  }

  .action-btn:active {
    transform: translateY(0);
  }

  /* Quota System */
  .quota-system {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .quota-meta {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .quota-label {
    font-size: 0.75rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.6);
  }

  .quota-val {
    font-family: var(--font-mono, monospace);
    font-size: 0.8rem;
    color: var(--color-primary, #00f3ff);
    font-weight: 700;
  }

  .quota-track {
    height: 8px;
    background: rgba(0, 0, 0, 0.4);
    border-radius: 4px;
    overflow: hidden;
  }

  .quota-fill {
    height: 100%;
    background: linear-gradient(90deg, #00f3ff 0%, #4facfe 100%);
    border-radius: 4px;
    position: relative;
    transition: width 0.8s cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: 0 0 15px rgba(0, 243, 255, 0.4);
  }

  .shimmer {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.3),
      transparent
    );
    transform: translateX(-100%);
    animation: shimmer 2s infinite;
  }

  @keyframes shimmer {
    100% {
      transform: translateX(100%);
    }
  }

  .quota-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.25rem;
    font-family: var(--font-mono, monospace);
    font-size: 0.7rem;
    color: rgba(255, 255, 255, 0.4);
  }

  .quota-footer .used {
    color: #fff;
    font-weight: 600;
  }

  .quota-footer .separator {
    color: rgba(255, 255, 255, 0.3);
  }

  /* Compact Mode */
  .compact {
    padding: 1.25rem;
    border-radius: 12px;
  }

  .compact .identity-header {
    margin-bottom: 1rem;
    gap: 0.75rem;
  }

  .compact .email {
    font-size: 0.85rem;
  }

  .compact .quota-track {
    height: 6px;
  }
</style>
