<script lang="ts">
  import { goto } from "$app/navigation";
  import Badge from "../ui/Badge.svelte";
  import Button from "../ui/Button.svelte";
  import { toasts } from "$lib/stores/toasts";

  interface Props {
    // TMDB data
    id?: number;
    title: string;
    posterPath?: string | null;
    posterUrl?: string | null;
    voteAverage?: number;
    releaseDate?: string;
    mediaType?: "movie" | "tv";

    // Fshare data
    fcode: string;
    originalFilename: string;
    fileSize: number;
    score: number;

    // Quality attributes (Enhanced API V3)
    quality?: string; // "1080p BluRay"
    resolution?: string; // "1080p"
    source?: string; // "BluRay"
    episodeTag?: string; // "S01E01"
    hasVietsub?: boolean;
    hasVietdub?: boolean;

    // Actions
    onDownload?: () => void;
    onCopyUrl?: () => void;
    onClick?: () => void;
  }

  let {
    id,
    title,
    posterPath,
    posterUrl,
    voteAverage = 0,
    releaseDate = "",
    mediaType = "movie",
    fcode,
    originalFilename,
    fileSize,
    score,
    quality,
    resolution,
    source,
    episodeTag,
    hasVietsub = false,
    hasVietdub = false,
    onDownload,
    onCopyUrl,
    onClick,
  }: Props = $props();

  function getPosterUrl(): string {
    if (posterPath) return `https://image.tmdb.org/t/p/w342${posterPath}`;
    if (posterUrl) return posterUrl;
    return "/images/placeholder-poster.svg";
  }

  function getYear(date: string): string {
    return date?.substring(0, 4) || "";
  }

  function formatSize(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  // V3 API handles scoring internally, so we rely on that or use default color
  function getScoreColor(val: number): string {
    return "high"; // Default to high/neon for now as visual preference
  }

  function handleCardClick() {
    if (onClick) {
      onClick();
    } else if (id) {
      goto(`/${mediaType}/${id}`);
    }
  }

  function handleDownload(e: Event) {
    e.stopPropagation();
    if (onDownload) onDownload();
  }

  function handleCopyUrl(e: Event) {
    e.stopPropagation();
    if (onCopyUrl) {
      onCopyUrl();
    } else {
      const url = `https://fshare.vn/file/${fcode}`;
      navigator.clipboard
        .writeText(url)
        .then(() => {
          toasts.success(`Link copied — ${url}`);
        })
        .catch(() => {
          toasts.error("Failed to copy link to clipboard");
        });
    }
  }
</script>

<div
  class="search-result-card-v3"
  role="button"
  tabindex="0"
  onclick={handleCardClick}
  onkeydown={(e) => e.key === "Enter" && handleCardClick()}
>
  <div class="card-inner">
    <img src={getPosterUrl()} alt={title} loading="lazy" />
    <div class="card-shine"></div>

    <!-- Episode Tag (Top Right - Prominent) -->
    {#if episodeTag}
      <div class="episode-badge">
        {episodeTag}
      </div>
    {/if}

    <!-- Quality Tags (Top Left) -->
    <div class="quality-tags">
      {#if resolution}
        <Badge text={resolution} variant="quality" size="sm" />
      {/if}
      {#if source}
        <Badge text={source} variant="source" size="sm" />
      {/if}
      {#if hasVietsub}
        <Badge text="Vietsub" variant="language" size="sm" color="#ff6b6b" />
      {/if}
      {#if hasVietdub}
        <Badge text="Vietdub" variant="language" size="sm" color="#ffa500" />
      {/if}
    </div>

    <!-- Card Overlay -->
    <div class="card-overlay">
      <div class="overlay-top">
        <h3 class="card-title">{title}</h3>
        <div class="card-meta">
          {#if releaseDate}
            <span class="meta-year">{getYear(releaseDate)}</span>
          {/if}
          <span class="meta-rating"
            ><span
              class="material-icons"
              style="font-size:0.75rem;vertical-align:middle;color:#f59e0b"
              >star</span
            >
            {voteAverage?.toFixed(1) || "N/A"}</span
          >
        </div>
      </div>

      <div class="overlay-bottom">
        <div class="file-info">
          <span class="material-icons info-icon">storage</span>
          <span class="file-size">{formatSize(fileSize)}</span>
        </div>

        <!-- Action Buttons -->
        <div class="card-actions">
          <Button size="sm" icon="download" onclick={handleDownload}
            >Download</Button
          >
          <Button
            size="sm"
            variant="ghost"
            icon="link"
            onclick={handleCopyUrl}
            title="Copy URL"
          ></Button>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .search-result-card-v3 {
    position: relative;
    aspect-ratio: 2/3;
    width: 100%;
    cursor: pointer;
    transition: transform 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
    z-index: 1;
  }

  .card-inner {
    position: relative;
    width: 100%;
    height: 100%;
    border-radius: 16px;
    overflow: hidden;
    background: #0a0f1e;
    border: 1px solid rgba(255, 255, 255, 0.05);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    transition: all 0.4s ease;
  }

  .search-result-card-v3:hover {
    transform: scale(1.05) translateY(-5px);
    z-index: 10;
  }

  .search-result-card-v3:hover .card-inner {
    border-color: rgba(0, 243, 255, 0.4);
    box-shadow: 0 20px 50px -10px rgba(0, 243, 255, 0.25);
  }

  .card-inner img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.6s ease;
  }

  .search-result-card-v3:hover img {
    transform: scale(1.1);
  }

  /* Shine */
  .card-shine {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      135deg,
      transparent 0%,
      rgba(255, 255, 255, 0.05) 50%,
      transparent 100%
    );
    transform: translateX(-100%);
    transition: transform 0.6s ease;
    z-index: 1;
  }

  .search-result-card-v3:hover .card-shine {
    transform: translateX(100%);
  }

  /* Episode Badge */
  .episode-badge {
    position: absolute;
    top: 1rem;
    right: 1rem;
    background: rgba(138, 43, 226, 0.85);
    color: #fff;
    font-size: 0.8rem;
    font-weight: 800;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    backdrop-filter: blur(8px);
    border: 1px solid rgba(255, 255, 255, 0.2);
    z-index: 5;
    box-shadow: 0 5px 15px rgba(0, 0, 0, 0.3);
  }

  /* Quality Tags */
  .quality-tags {
    position: absolute;
    top: 1rem;
    left: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    z-index: 5;
    align-items: flex-start;
  }

  /* Overlay System */
  .card-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 1.25rem;
    background: linear-gradient(
      to top,
      rgba(10, 15, 30, 0.98) 0%,
      rgba(10, 15, 30, 0.7) 40%,
      transparent 100%
    );
    z-index: 2;
    transition: all 0.3s ease;
  }

  .search-result-card-v3:hover .card-overlay {
    background: linear-gradient(
      to top,
      rgba(10, 15, 30, 1) 0%,
      rgba(10, 15, 30, 0.8) 60%
    );
  }

  .overlay-top {
    transform: translateY(0);
    transition: transform 0.4s ease;
  }

  .search-result-card-v3:hover .overlay-top {
    transform: translateY(-8px);
  }

  .card-title {
    margin: 0 0 0.4rem;
    font-size: 1rem;
    font-weight: 800;
    color: #fff;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .card-meta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.7);
    margin-bottom: 1rem;
  }

  .meta-year {
    color: var(--color-primary, #00f3ff);
    font-weight: 700;
    font-family: var(--font-mono, monospace);
  }

  .overlay-bottom {
    max-height: 0;
    opacity: 0;
    overflow: hidden;
    transition: all 0.4s ease;
  }

  .search-result-card-v3:hover .overlay-bottom {
    max-height: 120px;
    opacity: 1;
    margin-top: 0.5rem;
  }

  .file-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.5);
    margin-bottom: 1rem;
  }

  .info-icon {
    font-size: 14px;
  }
  .file-size {
    font-family: var(--font-mono, monospace);
    font-weight: 600;
  }

  /* Action Buttons */
  .card-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    padding-inline: 0.75rem;
  }
  /* Download button fills available space; copy stays compact */
  .card-actions :global(.flasharr-btn:first-child) {
    flex: 1;
  }

  .btn-download,
  .btn-copy {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 36px;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn-download {
    flex: 1;
    background: linear-gradient(
      135deg,
      var(--color-primary, #00f3ff) 0%,
      #00c8ff 100%
    );
    color: #000;
    font-size: 0.7rem;
    font-weight: 900;
    gap: 0.4rem;
    clip-path: polygon(
      8px 0%,
      100% 0%,
      100% calc(100% - 8px),
      calc(100% - 8px) 100%,
      0% 100%,
      0% 8px
    );
  }

  .btn-download:hover {
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.6);
    transform: translateY(-2px);
    filter: brightness(1.1);
  }

  .btn-copy {
    width: 36px;
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
    border: 1px solid rgba(255, 255, 255, 0.1);
    clip-path: polygon(
      4px 0%,
      calc(100% - 4px) 0%,
      100% 4px,
      100% calc(100% - 4px),
      calc(100% - 4px) 100%,
      4px 100%,
      0% calc(100% - 4px),
      0% 4px
    );
  }

  .btn-copy:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.3);
    color: #fff;
  }

  .btn-download .material-icons,
  .btn-copy .material-icons {
    font-size: 18px;
  }
</style>
