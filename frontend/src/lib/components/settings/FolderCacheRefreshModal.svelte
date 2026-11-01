<script lang="ts">
  import { Button, Modal } from "@media-set/core-ui";

  interface Props {
    open: boolean;
    onclose?: () => void;
    /** Starts the refresh; resolves to an error message, or nothing on success. */
    onsubmit: (url: string) => Promise<string | null | undefined> | string | null | undefined;
  }

  let { open, onclose, onsubmit }: Props = $props();

  let url = $state("");
  let submitting = $state(false);
  let error = $state("");

  async function submit() {
    if (submitting) return;
    const value = url.trim();
    try {
      const parsed = new URL(value);
      if (
        parsed.protocol !== "https:" ||
        !["fshare.vn", "www.fshare.vn"].includes(parsed.hostname) ||
        !parsed.pathname.startsWith("/folder/")
      ) {
        error = "Enter an HTTPS Fshare folder URL.";
        return;
      }
    } catch {
      error = "Enter a valid Fshare folder URL.";
      return;
    }
    submitting = true;
    error = (await onsubmit(value)) ?? "";
    submitting = false;
  }
</script>

<Modal {open} onClose={onclose} maxWidth="520px" ariaLabel="Refresh one Fshare folder">
  {#snippet header()}
    <div class="title"><span class="material-icons">folder_sync</span><div><b>REFRESH ONE FOLDER</b><small>Updates only this folder’s cached subtree.</small></div></div>
    <button class="close-btn" onclick={onclose} aria-label="Close"><span class="material-icons">close</span></button>
  {/snippet}

  <form
    class="content"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <label for="folder-refresh-url">Fshare folder URL</label>
    <input id="folder-refresh-url" bind:value={url} placeholder="https://www.fshare.vn/folder/..." disabled={submitting} />
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    <p>Existing cache entries outside this folder remain unchanged.</p>
    <div class="actions">
      <Button variant="ghost" size="sm" onclick={onclose} disabled={submitting}>CANCEL</Button>
      <Button accent="#00f3ff" size="sm" type="submit" disabled={submitting}>{submitting ? "STARTING…" : "REFRESH"}</Button>
    </div>
  </form>
</Modal>

<style>
  .title {
    display: flex;
    align-items: center;
    gap: .7rem;
    color: #fff;
  }

  .title > .material-icons {
    color: #00f3ff;
  }

  .title b {
    display: block;
    letter-spacing: .1em;
    font-size: .8rem;
  }

  .title small {
    display: block;
    margin-top: .15rem;
    color: #ffffff8c;
    font-size: .7rem;
  }

  .content {
    display: grid;
    gap: .7rem;
    color: #fffc;
  }

  label {
    font-size: .78rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: .08em;
  }

  input {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid rgba(0,243,255,.25);
    background: #00000040;
    color: #fff;
    padding: .7rem;
    font: inherit;
  }

  .content p {
    margin: 0;
    font-size: .78rem;
    color: #ffffff8c;
  }

  .error {
    color: #ff9da8 !important;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: .5rem;
    margin-top: .4rem;
  }

</style>
