# Flasharr Release Checklist

Use this checklist before tagging a release. The goal is to prove the full
download-to-library flow works without one-off path repairs, manual imports, or
ad-hoc permission changes.

## Release Blockers

- [ ] `cargo check` passes for the backend.
- [ ] Backend tests pass, or every failing test is triaged as unrelated to the release.
- [ ] Frontend build passes.
- [ ] Docker image builds from a clean checkout.
- [ ] Container starts as `PUID:PGID` with `UMASK=002`; `FLASHARR_RUN_AS_ROOT=true` is not required for normal deployments.
- [ ] `/appData`, `/downloads`, and `/data/flasharr-download` are writable by the configured app user.
- [ ] Sonarr and Radarr can see the same shared download path that Flasharr reports in SABnzbd history.
- [ ] Sonarr and Radarr import via `DownloadedEpisodesScan` / `DownloadedMoviesScan`; Flasharr does not need to create symlinks inside the media library for normal downloads.
- [ ] Import confirmation is closed-loop: Flasharr only sets `arr_announced=true` after Sonarr/Radarr reports the imported episode/movie.
- [ ] TMDB -> TVDB lookup is cached per title and not repeated for each episode.
- [ ] Smart Grab queues all valid episode matches for a TV title and does not queue weak unrelated matches.
- [ ] Movie grabs include TMDB metadata before they reach Radarr.
- [ ] Existing completed-but-unimported downloads can be backfilled without manual DB edits.

## E2E Smoke Test

- [ ] Remove the test title from Flasharr and Sonarr/Radarr.
- [ ] Smart Grab a TV title with more than one episode available.
- [ ] Confirm Flasharr creates one task per valid episode.
- [ ] Wait for downloads to complete.
- [ ] Confirm Sonarr imports every downloaded episode and marks each episode `hasFile=true`.
- [ ] Smart Grab or manually grab a movie.
- [ ] Confirm Radarr imports the movie and Flasharr records `arr_announced=true`.
- [ ] Confirm there are no stale import-pending queue items in Sonarr/Radarr.

## Deployment Checks

- [ ] `docker compose config` renders without errors.
- [ ] Health endpoint returns `{"status":"ok"}` at `/health` and `/api/health`.
- [ ] Container mount table shows the shared download folder mounted at:
  - `/downloads`
  - `/data/flasharr-download`
- [ ] A probe file written through one shared download alias is visible through the other alias.
- [ ] Flasharr Settings -> Downloads reports the shared path used by Sonarr/Radarr.
- [ ] Logs contain no recurring import path, permission, or queue mapping warnings.

## Release Notes

- [ ] Include any known test gaps.
- [ ] Include upgrade notes for `PUID`, `PGID`, `UMASK`, and shared download mounts.
- [ ] Include rollback steps for the Docker tag being released.
