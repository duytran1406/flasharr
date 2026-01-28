<script lang="ts">
  import { downloadStore } from "$lib/stores/downloads";
  import type { AddDownloadRequest } from "$lib/stores/downloads";

  interface Props {
    isOpen?: boolean;
    onClose?: () => void;
  }

  let { isOpen = $bindable(false), onClose }: Props = $props();

  let url = $state("");
  let filename = $state("");
  let category = $state("movies");
  let priority = $state<"NORMAL" | "HIGH" | "LOW">("NORMAL");
  let error = $state("");
  let isSubmitting = $state(false);
  let detectedHost = $state("");

  // Detect host from URL
  function detectHost(urlString: string): string {
    if (!urlString) return "";
    try {
      const urlObj = new URL(urlString);
      const hostname = urlObj.hostname.toLowerCase();

      if (hostname.includes("fshare.vn")) return "Fshare";
      if (hostname.includes("drive.google.com")) return "Google Drive";
      if (hostname.includes("mega.nz")) return "MEGA";
      if (hostname.includes("mediafire.com")) return "MediaFire";

      return "Unknown";
    } catch {
      return "";
    }
  }

  // Watch URL changes to detect host
  $effect(() => {
    detectedHost = detectHost(url);
  });

  // Validate URL
  function validateUrl(urlString: string): boolean {
    if (!urlString.trim()) {
      error = "URL is required";
      return false;
    }

    try {
      new URL(urlString);
    } catch {
      error = "Invalid URL format";
      return false;
    }

    const host = detectHost(urlString);
    if (host === "Unknown" || host === "") {
      error = "Unsupported host. Currently supported: Fshare";
      return false;
    }

    return true;
  }

  // Handle form submission
  async function handleSubmit() {
    error = "";

    if (!validateUrl(url)) {
      return;
    }

    isSubmitting = true;

    try {
      const request: AddDownloadRequest = {
        url: url.trim(),
        category: category || undefined,
        priority: priority,
      };

      // Add filename if provided
      if (filename.trim()) {
        request.filename = filename.trim();
      }

      const response = await downloadStore.addDownload(request);

      if (response.success) {
        // Success - close modal and reset form
        closeModal();
        resetForm();
      } else {
        error = response.error || "Failed to add download";
      }
    } catch (e: any) {
      error = e.message || "An unexpected error occurred";
    } finally {
      isSubmitting = false;
    }
  }

  // Close modal
  function closeModal() {
    isOpen = false;
    if (onClose) onClose();
  }

  // Reset form
  function resetForm() {
    url = "";
    filename = "";
    category = "movies";
    priority = "NORMAL";
    error = "";
    detectedHost = "";
  }

  // Handle escape key
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && isOpen) {
      closeModal();
    }
  }

  // Handle overlay click
  function handleOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      closeModal();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-overlay" onclick={handleOverlayClick}>
    <div
      class="modal-content"
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-title"
      tabindex="-1"
    >
      <div class="modal-header">
        <h2 id="modal-title">
          <span class="material-icons">add_circle</span>
          Add Download
        </h2>
        <button class="close-btn" onclick={closeModal} aria-label="Close modal">
          <span class="material-icons">close</span>
        </button>
      </div>

      <form
        onsubmit={(e) => {
          e.preventDefault();
          handleSubmit();
        }}
      >
        <div class="form-group">
          <label for="download-url">
            <span class="label-text">URL</span>
            <span class="required">*</span>
          </label>
          <input
            id="download-url"
            type="url"
            bind:value={url}
            placeholder="https://fshare.vn/file/..."
            required
            disabled={isSubmitting}
            autocomplete="off"
          />
          {#if detectedHost}
            <div class="host-badge" class:supported={detectedHost === "Fshare"}>
              <span class="material-icons">
                {detectedHost === "Fshare" ? "check_circle" : "warning"}
              </span>
              {detectedHost}
            </div>
          {/if}
        </div>

        <div class="form-group">
          <label for="download-filename">
            <span class="label-text">Filename</span>
            <span class="optional">(optional)</span>
          </label>
          <input
            id="download-filename"
            type="text"
            bind:value={filename}
            placeholder="movie.mkv"
            disabled={isSubmitting}
            autocomplete="off"
          />
          <div class="hint">Leave empty to use original filename</div>
        </div>

        <div class="form-row">
          <div class="form-group">
            <label for="download-category">
              <span class="label-text">Category</span>
            </label>
            <select
              id="download-category"
              bind:value={category}
              disabled={isSubmitting}
            >
              <option value="movies">Movies</option>
              <option value="tv">TV Shows</option>
              <option value="music">Music</option>
              <option value="other">Other</option>
            </select>
          </div>

          <div class="form-group">
            <label for="download-priority">
              <span class="label-text">Priority</span>
            </label>
            <select
              id="download-priority"
              bind:value={priority}
              disabled={isSubmitting}
            >
              <option value="LOW">Low</option>
              <option value="NORMAL">Normal</option>
              <option value="HIGH">High</option>
            </select>
          </div>
        </div>

        {#if error}
          <div class="error-message">
            <span class="material-icons">error</span>
            <span>{error}</span>
          </div>
        {/if}

        <div class="modal-actions">
          <button
            type="button"
            class="btn-secondary"
            onclick={closeModal}
            disabled={isSubmitting}
          >
            CANCEL
          </button>
          <button type="submit" class="btn-primary" disabled={isSubmitting}>
            {#if isSubmitting}
              <span class="spinner"></span>
              INITIALIZING...
            {:else}
              <span class="material-icons">download</span>
              START DOWNLOAD
            {/if}
          </button>
        </div>
      </form>

      <div class="keyboard-hint">
        <span class="material-icons">keyboard</span>
        Press <kbd>Esc</kbd> to close
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(8px);
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    opacity: 1;
    visibility: visible;
    animation: fadeIn 0.2s ease;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .modal-content {
    background: linear-gradient(
      135deg,
      rgba(26, 26, 46, 0.95),
      rgba(38, 38, 58, 0.95)
    );
    border: 1px solid rgba(0, 243, 255, 0.2);
    border-radius: 16px;
    padding: 2rem;
    max-width: 600px;
    width: 100%;
    max-height: 90vh;
    overflow-y: auto;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    animation: slideUp 0.3s ease;
  }

  @keyframes slideUp {
    from {
      transform: translateY(20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 2rem;
  }

  .modal-header h2 {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--color-primary, #00f3ff);
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--text-muted, #888);
    cursor: pointer;
    padding: 0.5rem;
    border-radius: 8px;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary, #fff);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .form-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  label {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--text-primary, #fff);
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .required {
    color: #ff0064;
  }

  .optional {
    color: var(--text-muted, #888);
    font-weight: 400;
    font-size: 0.75rem;
  }

  input,
  select {
    background: rgba(0, 0, 0, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 0.8rem 1.2rem;
    color: var(--text-primary, #fff);
    font-size: 0.875rem;
    font-family: var(--font-mono, monospace);
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    clip-path: polygon(
      0% 0%,
      calc(100% - 10px) 0%,
      100% 10px,
      100% 100%,
      10px 100%,
      0% calc(100% - 10px)
    );
  }

  input:focus,
  select:focus {
    outline: none;
    border-color: var(--color-primary, #00f3ff);
    background: rgba(0, 243, 255, 0.05);
    box-shadow: 0 0 20px rgba(0, 243, 255, 0.1);
  }

  .hint {
    font-size: 0.65rem;
    color: var(--text-muted, #888);
    margin-top: 0.25rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .host-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.85rem;
    font-size: 0.65rem;
    font-weight: 900;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    background: rgba(255, 204, 0, 0.1);
    border: 1px solid rgba(255, 204, 0, 0.3);
    color: #ffcc00;
    margin-top: 0.75rem;
    clip-path: polygon(8px 0%, 100% 0%, 100% 100%, 0% 100%, 0% 8px);
  }

  .host-badge.supported {
    background: rgba(0, 255, 128, 0.1);
    border-color: rgba(0, 255, 128, 0.3);
    color: #00ff80;
  }

  .host-badge .material-icons {
    font-size: 1rem;
  }

  .error-message {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: rgba(255, 0, 100, 0.1);
    border: 1px solid rgba(255, 0, 100, 0.3);
    border-radius: 8px;
    color: #ff0064;
    font-size: 0.875rem;
  }

  .error-message .material-icons {
    font-size: 1.25rem;
  }

  .modal-actions {
    display: flex;
    gap: 1rem;
    margin-top: 1rem;
  }

  .btn-secondary,
  .btn-primary {
    flex: 1;
    padding: 0.8rem 1.5rem;
    font-size: 0.75rem;
    font-weight: 900;
    letter-spacing: 0.15em;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    border: none;
    font-family: var(--font-mono, monospace);
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
    border: 1px solid rgba(255, 255, 255, 0.1);
    clip-path: polygon(
      8px 0%,
      100% 0%,
      100% calc(100% - 8px),
      calc(100% - 8px) 100%,
      0% 100%,
      0% 8px
    );
  }

  .btn-secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.2);
  }

  .btn-primary {
    background: linear-gradient(135deg, #0a1018 0%, #152030 50%, #0a1018 100%);
    color: var(--color-primary);
    border: 2px solid var(--color-primary);
    position: relative;
    overflow: hidden;
    clip-path: polygon(
      8px 0%,
      100% 0%,
      100% calc(100% - 8px),
      calc(100% - 8px) 100%,
      0% 100%,
      0% 8px
    );
  }

  .btn-primary::before {
    content: "";
    position: absolute;
    inset: -2px;
    background: linear-gradient(
      90deg,
      transparent 0%,
      rgba(0, 243, 255, 0.2) 45%,
      rgba(0, 243, 255, 0.6) 50%,
      rgba(0, 243, 255, 0.2) 55%,
      transparent 100%
    );
    animation: scan 3s linear infinite;
    opacity: 0;
    transition: opacity 0.3s;
    pointer-events: none;
  }

  .btn-primary:hover:not(:disabled) {
    box-shadow: 0 0 25px rgba(0, 243, 255, 0.4);
    color: #fff;
    border-color: #fff;
    transform: translateY(-2px);
  }

  .btn-primary:hover:not(:disabled)::before {
    opacity: 1;
  }

  .btn-primary:disabled,
  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(0, 0, 0, 0.3);
    border-top-color: #000;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .keyboard-hint {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    margin-top: 1.5rem;
    padding-top: 1.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    font-size: 0.75rem;
    color: var(--text-muted, #888);
  }

  .keyboard-hint .material-icons {
    font-size: 1rem;
  }

  kbd {
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 4px;
    padding: 0.125rem 0.5rem;
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    color: var(--text-primary, #fff);
  }

  @media (max-width: 640px) {
    .modal-content {
      padding: 1.5rem;
    }

    .form-row {
      grid-template-columns: 1fr;
    }

    .modal-actions {
      flex-direction: column;
    }
  }
</style>
