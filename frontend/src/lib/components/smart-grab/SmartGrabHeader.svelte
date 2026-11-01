<script lang="ts">
  import { Badge } from "@media-set/core-ui";

  interface Props {
    title: string;
    downloadedCount: number;
    skipDownloaded: boolean;
    disabled?: boolean;
    onSkipToggle: (skip: boolean) => void;
    onClose?: () => void;
  }

  let { title, downloadedCount, skipDownloaded, disabled = false, onSkipToggle, onClose }: Props = $props();
</script>

<div class="header-info">
  <Badge text="SMART GRAB" variant="smart" size="sm" />
  <h2>{title}</h2>
  {#if downloadedCount > 0}
    <div class="downloaded-controls">
      <span class="downloaded-info"
        ><span class="material-icons">check_circle</span>{downloadedCount} already downloaded</span
      >
      <label class="toggle-switch">
        <input
          type="checkbox"
          checked={skipDownloaded}
          {disabled}
          onchange={(event) => onSkipToggle(event.currentTarget.checked)}
        />
        <span class="toggle-slider"></span>
        <span class="toggle-label">{skipDownloaded ? "Missing only" : "Grab all"}</span>
      </label>
    </div>
  {/if}
</div>
<button class="close-btn" type="button" onclick={onClose} aria-label="Close smart grab">
  <span class="material-icons">close</span>
</button>

<style>
  .downloaded-info {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: .75rem;
    font-weight: 600;
    color: #10b981;
    background: #10b9811a;
    border: 1px solid rgba(16,185,129,.2);
    padding: 3px 10px;
    border-radius: 6px;
    margin-top: 6px;
  }

  .downloaded-info .material-icons {
    font-size: .85rem;
  }

  .downloaded-controls {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 8px;
    flex-wrap: wrap;
  }

  .toggle-switch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    -webkit-user-select: none;
    user-select: none;
  }

  .toggle-switch input {
    display: none;
  }

  .toggle-switch:has(input:disabled) {
    cursor: not-allowed;
    opacity: .6;
  }

  .toggle-slider {
    width: 36px;
    height: 20px;
    background: #ffffff26;
    border-radius: 10px;
    position: relative;
    transition: background .2s;
  }

  .toggle-slider:after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    background: #fff;
    border-radius: 50%;
    transition: transform .2s;
  }

  .toggle-switch input:checked + .toggle-slider {
    background: #10b981;
  }

  .toggle-switch input:checked + .toggle-slider:after {
    transform: translate(16px);
  }

  .toggle-label {
    font-size: .7rem;
    font-weight: 600;
    color: #ffffffb3;
    letter-spacing: .02em;
  }

  .close-btn {
    background: #ffffff0d;
    border: none;
    color: #fff6;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all .2s;
  }

  .close-btn:hover {
    background: #ffffff1a;
    color: #fff;
  }

  h2 {
    margin: .5rem 0 0;
    font-size: 1.05rem;
    font-weight: 800;
    color: #fff;
  }
</style>
