<script lang="ts">
  import { Button } from "@media-set/core-ui";
  import {
    integrationStatusKind,
    integrationStatusLabel,
    type IntegrationHealth,
  } from "$lib/stores/system";
  import type { LibraryOverview } from "$lib/stores/arr";

  interface Props {
    indexerApiKey: string;
    showIndexerApiKey?: boolean;
    sonarrUrl: string;
    sonarrApiKey: string;
    showSonarrApiKey?: boolean;
    sonarrStatus?: IntegrationHealth | null;
    sonarrTesting?: boolean;
    sonarrSaving?: boolean;
    sonarrChangedElsewhere?: boolean;
    radarrUrl: string;
    radarrApiKey: string;
    showRadarrApiKey?: boolean;
    radarrStatus?: IntegrationHealth | null;
    radarrTesting?: boolean;
    radarrSaving?: boolean;
    radarrChangedElsewhere?: boolean;
    libraryOverview?: LibraryOverview | null;
    onCopy: (value: string, label: string) => void;
    onGenerateKey: () => void;
    onToggleIndexerKey: () => void;
    onToggleSonarrKey: () => void;
    onToggleRadarrKey: () => void;
    onTestSonarr: () => void;
    onSaveSonarr: () => void;
    onReloadSonarr?: () => void;
    onTestRadarr: () => void;
    onSaveRadarr: () => void;
    onReloadRadarr?: () => void;
  }

  function formatLibrarySize(bytes: number): string {
    const mb = bytes / 1048576;
    if (mb < 1024) return `${mb.toFixed(1)} MB`;
    const gb = mb / 1024;
    return gb < 1024 ? `${gb.toFixed(1)} GB` : `${(gb / 1024).toFixed(1)} TB`;
  }

  let {
    indexerApiKey = $bindable(),
    showIndexerApiKey = false,
    sonarrUrl = $bindable(),
    sonarrApiKey = $bindable(),
    showSonarrApiKey = false,
    sonarrStatus = null,
    sonarrTesting = false,
    sonarrSaving = false,
    sonarrChangedElsewhere = false,
    radarrUrl = $bindable(),
    radarrApiKey = $bindable(),
    showRadarrApiKey = false,
    radarrStatus = null,
    radarrTesting = false,
    radarrSaving = false,
    radarrChangedElsewhere = false,
    libraryOverview = null,
    onCopy,
    onGenerateKey,
    onToggleIndexerKey,
    onToggleSonarrKey,
    onToggleRadarrKey,
    onTestSonarr,
    onSaveSonarr,
    onReloadSonarr,
    onTestRadarr,
    onSaveRadarr,
    onReloadRadarr,
  }: Props = $props();
</script>

<div class="integration-grid" aria-label="Integrations">
  <section class="bento-card bento-newznab">
    <div class="bento-head violet-accent"><img src="/images/integrations/newznab-logo.png" alt="" class="head-logo" /><h3>NEWZNAB</h3></div>
    <div class="card-banner violet-banner"><div class="banner-dots"></div><div class="banner-logo-ring glow"><img src="/images/integrations/newznab-logo.png" alt="Newznab" /></div></div>
    <div class="bento-body">
      {#each [["URL", "http://flasharr:8484/newznab/api", "Endpoint"], ["USER", "flasharr", "Username"], ["PASS", "flasharr-pwd", "Password"]] as field}
        <div class="inline-field"><span class="inline-label">{field[0]}</span><span class="inline-value">{field[1]}</span><button class="copy-inline-btn" aria-label={`Copy ${field[2]}`} onclick={() => onCopy(field[1], field[2])}><span class="material-icons" aria-hidden="true">content_copy</span></button></div>
      {/each}
      <div class="inline-field"><span class="inline-label">KEY</span><span class="inline-value mono">{indexerApiKey ? (showIndexerApiKey ? indexerApiKey : "••••••••") : "(no key — generating…)"}</span><button class="copy-inline-btn" aria-label={showIndexerApiKey ? "Hide indexer API key" : "Reveal indexer API key"} aria-pressed={showIndexerApiKey} onclick={onToggleIndexerKey}><span class="material-icons" aria-hidden="true">{showIndexerApiKey ? "visibility_off" : "visibility"}</span></button><button class="copy-inline-btn" aria-label="Copy API key" onclick={() => onCopy(indexerApiKey, "API Key")}><span class="material-icons" aria-hidden="true">content_copy</span></button></div>
    </div>
    <div class="acc-card-actions"><div style="flex:1;display:flex"><Button accent="#8b5cf6" icon="autorenew" size="sm" width="100%" onclick={onGenerateKey}>Regen Key</Button></div></div>
  </section>

  <section class="bento-card bento-sonarr">
    <div class="bento-head sky-accent"><img src="/images/integrations/sonarr.png" alt="" class="head-logo" /><h3>SONARR</h3></div>
    <div class="card-banner sky-banner"><div class="banner-dots"></div><div class="banner-logo-ring glow"><img src="/images/integrations/sonarr.png" alt="Sonarr" /><span
          class="banner-status-dot"
          class:active={integrationStatusKind(sonarrStatus, sonarrTesting || sonarrSaving) === "connected"}
          class:unreachable={integrationStatusKind(sonarrStatus, sonarrTesting || sonarrSaving) === "unreachable"}
          role="status"
          aria-label={`Sonarr: ${integrationStatusLabel(sonarrStatus, sonarrTesting || sonarrSaving)}`}
        ></span></div></div>
    <div class="bento-body">
      {#if sonarrChangedElsewhere}<button class="reload-config" onclick={onReloadSonarr}>Configuration changed elsewhere · Reload</button>{/if}<div class="inline-field"><label class="inline-label" for="sonarr-url">URL</label><input id="sonarr-url" type="text" bind:value={sonarrUrl} placeholder="http://localhost:8989" class="inline-input-edit" /></div><div class="inline-field"><label class="inline-label" for="sonarr-key">KEY</label><input id="sonarr-key" type={showSonarrApiKey ? "text" : "password"} bind:value={sonarrApiKey} placeholder="API Key" class="inline-input-edit mono" /><button class="copy-inline-btn" aria-label={showSonarrApiKey ? "Hide Sonarr API key" : "Reveal Sonarr API key"} aria-pressed={showSonarrApiKey} onclick={onToggleSonarrKey}><span class="material-icons" aria-hidden="true">{showSonarrApiKey ? "visibility_off" : "visibility"}</span></button></div><div class="library-status" aria-label="Sonarr library status">{#if libraryOverview?.sonarr_connected}<div class="library-metric"><b>{libraryOverview.series_count}</b><small>SERIES</small></div><div class="library-metric"><b>{libraryOverview.episodes_with_files}/{libraryOverview.total_episodes}</b><small>EPISODES</small></div><div class="library-metric missing"><b>{libraryOverview.episodes_missing}</b><small>MISSING</small></div><div class="library-metric"><b>{formatLibrarySize(libraryOverview.series_size_on_disk)}</b><small>SIZE</small></div>{:else}<span class="library-unavailable">LIBRARY UNAVAILABLE</span>{/if}</div><div class="compact-actions"><div style="flex:4;display:flex"><Button variant="ghost" size="sm" width="100%" disabled={sonarrTesting || sonarrSaving} onclick={onTestSonarr}><span class="material-icons">{sonarrTesting ? "refresh" : "sync_alt"}</span> VERIFY</Button></div><div style="flex:6;display:flex"><Button accent="#38bdf8" size="sm" width="100%" loading={sonarrSaving} disabled={sonarrSaving} onclick={onSaveSonarr}>SAVE</Button></div></div>
    </div>
  </section>

  <section class="bento-card bento-sabnzbd">
    <div class="bento-head teal-accent"><img src="/images/integrations/sabnzbd.png" alt="" class="head-logo" /><h3>SABNZBD</h3></div>
    <div class="card-banner teal-banner"><div class="banner-dots"></div><div class="banner-logo-ring glow"><img src="/images/integrations/sabnzbd.png" alt="SABnzbd" /></div></div>
    <div class="bento-body">
      {#each [["URL", "http://flasharr:8484/sabnzbd/api", "Endpoint"], ["USER", "flasharr", "Username"], ["PASS", "flasharr-pwd", "Password"]] as field}
        <div class="inline-field"><span class="inline-label">{field[0]}</span><span class="inline-value">{field[1]}</span><button class="copy-inline-btn" aria-label={`Copy ${field[2]}`} onclick={() => onCopy(field[1], field[2])}><span class="material-icons" aria-hidden="true">content_copy</span></button></div>
      {/each}
      <div class="inline-field"><span class="inline-label">KEY</span><span class="inline-value mono">{indexerApiKey ? (showIndexerApiKey ? indexerApiKey : "••••••••") : "(no key — generating…)"}</span><button class="copy-inline-btn" aria-label={showIndexerApiKey ? "Hide indexer API key" : "Reveal indexer API key"} aria-pressed={showIndexerApiKey} onclick={onToggleIndexerKey}><span class="material-icons" aria-hidden="true">{showIndexerApiKey ? "visibility_off" : "visibility"}</span></button><button class="copy-inline-btn" aria-label="Copy API key" onclick={() => onCopy(indexerApiKey, "API Key")}><span class="material-icons" aria-hidden="true">content_copy</span></button></div>
    </div>
    <div class="acc-card-actions"><div style="flex:1;display:flex"><Button accent="#00d4aa" icon="autorenew" size="sm" width="100%" onclick={onGenerateKey}>Regen Key</Button></div></div>
  </section>

  <section class="bento-card bento-radarr">
    <div class="bento-head amber-accent"><img src="/images/integrations/radarr.png" alt="" class="head-logo" /><h3>RADARR</h3></div>
    <div class="card-banner amber-banner"><div class="banner-dots"></div><div class="banner-logo-ring glow"><img src="/images/integrations/radarr.png" alt="Radarr" /><span
          class="banner-status-dot"
          class:active={integrationStatusKind(radarrStatus, radarrTesting || radarrSaving) === "connected"}
          class:unreachable={integrationStatusKind(radarrStatus, radarrTesting || radarrSaving) === "unreachable"}
          role="status"
          aria-label={`Radarr: ${integrationStatusLabel(radarrStatus, radarrTesting || radarrSaving)}`}
        ></span></div></div>
    <div class="bento-body">
      {#if radarrChangedElsewhere}<button class="reload-config" onclick={onReloadRadarr}>Configuration changed elsewhere · Reload</button>{/if}<div class="inline-field"><label class="inline-label" for="radarr-url">URL</label><input id="radarr-url" type="text" bind:value={radarrUrl} placeholder="http://localhost:7878" class="inline-input-edit" /></div><div class="inline-field"><label class="inline-label" for="radarr-key">KEY</label><input id="radarr-key" type={showRadarrApiKey ? "text" : "password"} bind:value={radarrApiKey} placeholder="API Key" class="inline-input-edit mono" /><button class="copy-inline-btn" aria-label={showRadarrApiKey ? "Hide Radarr API key" : "Reveal Radarr API key"} aria-pressed={showRadarrApiKey} onclick={onToggleRadarrKey}><span class="material-icons" aria-hidden="true">{showRadarrApiKey ? "visibility_off" : "visibility"}</span></button></div><div class="library-status" aria-label="Radarr library status">{#if libraryOverview?.radarr_connected}<div class="library-metric"><b>{libraryOverview.movie_count}</b><small>MOVIES</small></div><div class="library-metric"><b>{libraryOverview.movies_with_files}</b><small>DOWNLOADED</small></div><div class="library-metric missing"><b>{libraryOverview.movies_missing}</b><small>MISSING</small></div><div class="library-metric"><b>{formatLibrarySize(libraryOverview.movies_size_on_disk)}</b><small>SIZE</small></div>{:else}<span class="library-unavailable">LIBRARY UNAVAILABLE</span>{/if}</div><div class="compact-actions"><div style="flex:4;display:flex"><Button variant="ghost" size="sm" width="100%" disabled={radarrTesting || radarrSaving} onclick={onTestRadarr}><span class="material-icons">{radarrTesting ? "refresh" : "sync_alt"}</span> VERIFY</Button></div><div style="flex:6;display:flex"><Button accent="#f59e0b" size="sm" width="100%" loading={radarrSaving} disabled={radarrSaving} onclick={onSaveRadarr}>SAVE</Button></div></div>
    </div>
  </section>
</div>

<style>
  .reload-config {
    width: fit-content;
    padding: 0;
    border: 0;
    background: none;
    color: #fbbf24;
    font-size: var(--cui-text-xs);
    text-align: left;
    cursor: pointer;
  }

  .reload-config:focus-visible {
    outline: 2px solid var(--cui-primary);
    outline-offset: 2px;
  }

  .integration-grid {
    display: grid;
    grid-template-columns: repeat(4,minmax(0,1fr));
    gap: .65rem;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .bento-card {
    --card-accent: rgba(255,255,255,.05);
    border-radius: 14px;
    background: #080a0fb3;
    backdrop-filter: blur(12px);
    border: 1px solid color-mix(in srgb,var(--card-accent) 35%,transparent);
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }

  .bento-newznab {
    --card-accent: rgba(139,92,246,.5);
  }

  .bento-sonarr {
    --card-accent: rgba(56,189,248,.5);
  }

  .bento-radarr {
    --card-accent: rgba(245,158,11,.5);
  }

  .bento-sabnzbd {
    --card-accent: rgba(0,212,170,.5);
  }

  .bento-head {
    display: flex;
    align-items: center;
    gap: .5rem;
    padding: .55rem .7rem;
    font-family: var(--cui-font-mono,monospace);
    flex-shrink: 0;
    position: relative;
    overflow: hidden;
    background: transparent;
  }

  .bento-head:after {
    content: "";
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(0,0,0,.25) 1px,transparent 1px);
    background-size: 6px 6px;
    mask-image: linear-gradient(to right,#000 0%,transparent 70%);
    -webkit-mask-image: linear-gradient(to right,#000 0%,transparent 70%);
  }

  .bento-head > * {
    position: relative;
    z-index: 1;
  }

  .bento-head h3 {
    font-size: .75rem;
    font-weight: 800;
    letter-spacing: .12em;
    color: #fff;
    margin: 0;
    flex: 1;
  }

  .head-logo {
    width: 22px;
    height: 22px;
    border-radius: 4px;
    object-fit: contain;
  }

  .violet-accent {
    border-left: 3px solid rgba(139,92,246,.5);
  }

  .sky-accent {
    border-left: 3px solid rgba(56,189,248,.5);
  }

  .amber-accent {
    border-left: 3px solid rgba(245,158,11,.5);
  }

  .teal-accent {
    border-left: 3px solid rgba(0,212,170,.5);
  }

  .card-banner {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 72px;
    flex-shrink: 0;
  }

  .banner-dots {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgba(0,0,0,.25) 1px,transparent 1px);
    background-size: 6px 6px;
  }

  .banner-logo-ring {
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    z-index: 1;
  }

  .banner-logo-ring.glow {
    border: 0;
    border-radius: 0;
    overflow: visible;
    background: transparent;
    box-shadow: none;
  }

  .banner-logo-ring.glow img {
    width: 44px;
    height: 44px;
    object-fit: contain;
  }

  .bento-sonarr .banner-logo-ring.glow img {
    width: 40px;
    height: 40px;
  }

  .banner-status-dot {
    position: absolute;
    right: -3px;
    bottom: -3px;
    width: 11px;
    height: 11px;
    border: 2px solid rgba(8,10,15,.95);
    border-radius: 50%;
    background: #50505ad9;
    z-index: 2;
  }

  .banner-status-dot.active {
    background: #22c55e;
    box-shadow: 0 0 8px #22c55ed9;
  }

  .banner-status-dot.unreachable {
    background: #ef4444;
    box-shadow: 0 0 8px #ef4444bf;
  }

  .violet-banner {
    background: linear-gradient(135deg,#8b5cf626,#643cc80f 50%,#0a0a0f80);
  }

  .sky-banner {
    background: linear-gradient(135deg,#38bdf826,#288cc80f 50%,#0a0a0f80);
  }

  .amber-banner {
    background: linear-gradient(135deg,#f59e0b26,#c8780a0f 50%,#0a0a0f80);
  }

  .teal-banner {
    background: linear-gradient(135deg,#00d4aa26,#00a0820f 50%,#0a0a0f80);
  }

  .violet-banner .banner-logo-ring.glow img {
    filter: drop-shadow(0 0 10px rgba(139,92,246,.7)) drop-shadow(0 0 20px rgba(139,92,246,.35));
  }

  .sky-banner .banner-logo-ring.glow img {
    filter: drop-shadow(0 0 10px rgba(56,189,248,.7)) drop-shadow(0 0 20px rgba(56,189,248,.35));
  }

  .amber-banner .banner-logo-ring.glow img {
    filter: drop-shadow(0 0 10px rgba(245,158,11,.7)) drop-shadow(0 0 20px rgba(245,158,11,.35));
  }

  .teal-banner .banner-logo-ring.glow img {
    filter: drop-shadow(0 0 10px rgba(0,212,170,.7)) drop-shadow(0 0 20px rgba(0,212,170,.35));
  }

  .bento-body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    padding: .6rem .85rem;
  }

  .inline-field {
    display: grid;
    grid-template-columns: minmax(0,1fr) auto auto;
    align-items: center;
    column-gap: .4rem;
    row-gap: .4rem;
    padding: .55rem 0;
    border-bottom: 1px solid rgba(255,255,255,.03);
  }

  .inline-label {
    grid-column: 1 / -1;
    font: 700 .72rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .1em;
    color: #ffffff59;
  }

  .inline-value {
    grid-column: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: .9rem;
    color: #fffc;
  }

  .mono {
    font-family: var(--cui-font-mono,monospace);
  }

  .inline-input-edit {
    grid-column: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    border-bottom: 1px solid rgba(255,255,255,.06);
    color: #fffc;
    font-size: .8rem;
    padding: .05rem .2rem;
    outline: none;
  }

  .copy-inline-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 44px;
    min-height: 44px;
    border: 0;
    background: none;
    color: var(--cui-text-secondary);
    cursor: pointer;
    padding: 2px;
  }

  .copy-inline-btn:hover {
    color: #00f3ff;
  }

  .copy-inline-btn .material-icons {
    font-size: .8rem;
  }

  .library-status {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: .45rem;
    padding: .65rem 0;
  }

  .library-metric {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: .45rem .5rem;
    background: #ffffff06;
    border: 1px solid rgba(255,255,255,.05);
    border-radius: 4px;
  }

  .library-metric b {
    color: #eaf6fa;
    font: 800 .8rem/1 var(--cui-font-mono,monospace);
  }

  .library-metric small {
    color: #fff6;
    font: 800 .5rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .1em;
  }

  .library-metric.missing b {
    color: #fbbf24;
  }

  .library-unavailable {
    grid-column: 1 / -1;
    color: #ffffff59;
    font: 800 .58rem/1 var(--cui-font-mono,monospace);
    letter-spacing: .1em;
  }

  .compact-actions {
    display: flex;
    gap: .4rem;
    margin-top: auto;
    padding-top: .6rem;
  }

  .acc-card-actions {
    display: flex;
    padding: .55rem .85rem .7rem;
  }

  .acc-card-actions :global(.flasharr-btn),
  .compact-actions :global(.flasharr-btn) {
    height: 44px;
    min-height: 44px;
    box-sizing: border-box;
  }

  .copy-inline-btn:focus-visible,
  .inline-input-edit:focus-visible {
    outline: 2px solid var(--cui-primary);
    outline-offset: 2px;
  }

  @media (prefers-reduced-motion:reduce) {
    .banner-status-dot {
      transition: none;
    }
  }

  @media (max-width:900px) {
    .integration-grid {
      height: auto;
      grid-template-columns: 1fr 1fr;
    }
    .bento-card {
      min-height: 280px;
    }
  }

  @media (max-width:560px) {
    .integration-grid {
      grid-template-columns: 1fr;
    }
  }

</style>
