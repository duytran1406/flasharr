# ⚡ Flasharr — Complete Installation Guide (Beginner-Friendly)

> **Estimated time:** 15–20 minutes  
> **Difficulty:** Easy — no prior Docker or Linux experience required  
> **Last updated:** February 2026

---

## 📑 Table of Contents

1. [What is Flasharr?](#-what-is-flasharr)
2. [Prerequisites](#-prerequisites)
3. [Step 1 — Install Docker](#-step-1--install-docker)
4. [Step 2 — Create the Project Folder](#-step-2--create-the-project-folder)
5. [Step 3 — Create the docker-compose.yml File](#-step-3--create-the-docker-composeyml-file)
6. [Step 4 — Customize the Download Path (Optional)](#-step-4--customize-the-download-path-optional)
7. [Step 5 — Start Flasharr](#-step-5--start-flasharr)
8. [Step 6 — First-Time Setup Wizard](#-step-6--first-time-setup-wizard)
9. [Step 7 — Add Flasharr to Sonarr (TV Shows)](#-step-7--add-flasharr-to-sonarr-tv-shows)
10. [Step 8 — Add Flasharr to Radarr (Movies)](#-step-8--add-flasharr-to-radarr-movies)
11. [Step 9 — Verify Everything Works](#-step-9--verify-everything-works)
12. [Optional — Auto-Update with Watchtower](#-optional--auto-update-with-watchtower)
13. [Updating Flasharr](#-updating-flasharr)
14. [Troubleshooting](#-troubleshooting)
15. [Useful Commands Cheat Sheet](#-useful-commands-cheat-sheet)

---

## 🤔 What is Flasharr?

Flasharr is a **blazing-fast download manager** built with Rust and SvelteKit. It downloads files from [Fshare.vn](https://www.fshare.vn/) and integrates seamlessly with **Sonarr** (for TV shows) and **Radarr** (for movies).

**How the integration works:**

```
┌──────────────┐    Search     ┌───────────┐   Download    ┌──────────┐
│ Sonarr/Radarr│──────────────▶│  Flasharr │──────────────▶│  Fshare  │
│    (*arr)    │◀──────────────│  (Manager)│◀──────────────│  (Host)  │
└──────────────┘   Results     └───────────┘   Files       └──────────┘
```

- **Newznab Indexer** — Sonarr/Radarr search for media through Flasharr
- **SABnzbd Download Client** — Sonarr/Radarr send download requests to Flasharr

> **Requirement:** You need an active **Fshare VIP account** to use Flasharr.

---

## 📋 Prerequisites

Before you begin, make sure you have:

| Item                             | Description                                                            | Where to Get It                                                         |
| -------------------------------- | ---------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| **A computer or server**         | Any machine running Linux, macOS, or Windows                           | —                                                                       |
| **Docker**                       | Container platform to run Flasharr                                     | [Get Docker](https://docs.docker.com/get-docker/)                       |
| **Docker Compose**               | Included with Docker Desktop (macOS/Windows). On Linux, it's a plugin. | [Install Compose](https://docs.docker.com/compose/install/)             |
| **Fshare VIP Account**           | Required for downloading files                                         | [fshare.vn](https://www.fshare.vn/)                                     |
| **Sonarr & Radarr** _(optional)_ | If you want automated media management                                 | [sonarr.tv](https://sonarr.tv/) / [radarr.video](https://radarr.video/) |

---

## 🐳 Step 1 — Install Docker

If you already have Docker installed, skip to [Step 2](#-step-2--create-the-project-folder).

### macOS / Windows

1. Download **Docker Desktop** from [https://docs.docker.com/get-docker/](https://docs.docker.com/get-docker/)
2. Run the installer and follow the on-screen instructions
3. Launch Docker Desktop — wait for the whale icon 🐳 to appear in your taskbar/menu bar
4. Open a terminal (Terminal on macOS, PowerShell on Windows) and verify:

```bash
docker --version
docker compose version
```

You should see version numbers for both commands.

### Linux (Ubuntu/Debian)

Run these commands one by one:

```bash
# Update your system
sudo apt update && sudo apt upgrade -y

# Install Docker
curl -fsSL https://get.docker.com | sudo sh

# Add your user to the docker group (so you don't need sudo)
sudo usermod -aG docker $USER

# Log out and log back in, then verify:
docker --version
docker compose version
```

> **💡 Tip:** If `docker compose version` doesn't work on Linux, you may need to install the Compose plugin:
>
> ```bash
> sudo apt install docker-compose-plugin
> ```

---

## 📁 Step 2 — Create the Project Folder

Choose a location on your computer where Flasharr will store its data. Open a terminal and run:

```bash
# Create a folder for Flasharr
mkdir -p ~/flasharr
cd ~/flasharr

# Create the data directory
mkdir -p appData
```

> **💡 What is `appData`?**  
> This folder stores Flasharr's database, configuration, and downloaded files. It persists even if you restart or remove the container.

---

## 📝 Step 3 — Create the docker-compose.yml File

While inside the `~/flasharr` folder, create the `docker-compose.yml` file.

### Option A: Download It Automatically

```bash
curl -O https://raw.githubusercontent.com/duytran1406/flasharr/main/docker-compose.production.yml
mv docker-compose.production.yml docker-compose.yml
```

### Option B: Create It Manually

Create a new file called `docker-compose.yml`:

```bash
nano docker-compose.yml
```

Paste the following content:

```yaml
version: "3.8"

services:
  flasharr:
    image: ghcr.io/duytran1406/flasharr:latest
    container_name: flasharr
    restart: unless-stopped

    ports:
      - "8484:8484"

    volumes:
      # Database & configuration (DO NOT CHANGE)
      - ./appData:/appData

      # Optional: Custom download location (see Step 4)
      # - /path/to/your/downloads:/appData/downloads

    environment:
      # Set your timezone (find yours: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones)
      - TZ=Asia/Bangkok

      # Logging level (keep as-is unless debugging)
      - RUST_LOG=flasharr=info,tower_http=info

      # Data directory (DO NOT CHANGE)
      - FLASHARR_APPDATA_DIR=/appData

    # Health check — Docker will monitor if Flasharr is running
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8484/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
```

Save the file:

- **nano:** Press `Ctrl + X`, then `Y`, then `Enter`
- **VS Code / other editor:** Just save normally

---

## 📂 Step 4 — Customize the Download Path (Optional)

By default, Flasharr saves downloads inside `./appData/downloads/`. If you want downloads saved to a **different location** on your computer (for example, an external hard drive or a NAS share), follow these steps:

### 4.1 — Choose Your Download Folder

Decide where you want your downloaded files to go. For example:

| OS                   | Example Path                                |
| -------------------- | ------------------------------------------- |
| Linux                | `/mnt/media/downloads` or `/data/downloads` |
| macOS                | `/Users/yourname/Media/downloads`           |
| Windows (WSL)        | `/mnt/c/Users/yourname/Downloads/flasharr`  |
| NAS / External Drive | `/mnt/nas/downloads`                        |

### 4.2 — Create the Folder

```bash
# Replace with YOUR actual path
sudo mkdir -p /mnt/media/downloads
```

### 4.3 — Edit docker-compose.yml

Open the file:

```bash
nano ~/flasharr/docker-compose.yml
```

Find this line:

```yaml
# - /path/to/your/downloads:/appData/downloads
```

**Remove the `#`** and **replace the path** with your actual folder:

```yaml
- /mnt/media/downloads:/appData/downloads
```

**Before (commented out = inactive):**

```yaml
# - /path/to/your/downloads:/appData/downloads
```

**After (active):**

```yaml
- /mnt/media/downloads:/appData/downloads
```

Save the file.

> **⚠️ Important:** The part **after** the colon (`:`) must stay as `/appData/downloads`. Only change the part **before** the colon to your local path.

### 4.4 — Understanding Volume Mounts

```
/mnt/media/downloads  :  /appData/downloads
├── Your computer     │  ├── Inside the container
│   (host path)       │  │   (container path)
│                     │  │
│   Files you see     │  │   Where Flasharr
│   on your disk  ◄───┼──┤   saves files
```

The colon `:` connects a folder on your computer (left side) to a folder inside the container (right side). They see the same files.

---

## 🚀 Step 5 — Start Flasharr

Make sure you're in the `~/flasharr` folder, then run:

```bash
cd ~/flasharr
docker compose up -d
```

**What this does:**

- `docker compose up` — Creates and starts the Flasharr container
- `-d` — Runs it in the background (detached mode)

You should see output like:

```
[+] Pulling flasharr ...
[+] Running 1/1
 ✔ Container flasharr  Started
```

### Verify It's Running

```bash
docker compose ps
```

You should see:

```
NAME       IMAGE                                    STATUS                   PORTS
flasharr   ghcr.io/duytran1406/flasharr:latest     Up X seconds (healthy)   0.0.0.0:8484->8484/tcp
```

### Access Flasharr

Open your web browser and go to:

```
http://localhost:8484
```

> **💡 Accessing from another device on your network?**  
> Use your computer's IP address instead of `localhost`:  
> `http://192.168.1.XXX:8484`
>
> Find your IP:
>
> - **Linux/macOS:** `hostname -I` or `ifconfig`
> - **Windows:** `ipconfig`

---

## 🧙 Step 6 — First-Time Setup Wizard

When you open Flasharr for the first time, the setup wizard will guide you through the initial configuration.

### 6.1 — Step 1: Fshare Credentials

Enter your Fshare account details:

| Field        | What to Enter                |
| ------------ | ---------------------------- |
| **Email**    | Your Fshare account email    |
| **Password** | Your Fshare account password |

Click **Test Connection** to verify your credentials, then proceed.

> **⚠️ Important:** You need a **Fshare VIP account** for downloads to work. Free accounts have severe speed and download limits.

### 6.2 — Step 2: Download Settings

Configure your preferred settings:

| Setting                      | Recommended Value | Description                                  |
| ---------------------------- | ----------------- | -------------------------------------------- |
| **Download Path**            | `/downloads`      | Where files are saved (inside the container) |
| **Max Concurrent Downloads** | 3–5               | How many files download at the same time     |

### 6.3 — Step 3: Integrations (Optional)

You can optionally connect Sonarr, Radarr, and Jellyfin to Flasharr during setup. You can also skip this and configure them later in **Settings**.

> **💡 Tip:** If you haven't set up Sonarr/Radarr yet, skip this step for now and come back to it after completing Steps 7 and 8 below.

### 6.4 — Get Your Flasharr API Key

After completing the setup, go to **Settings → Services** in Flasharr to find your **API Key**. You'll need this for Sonarr/Radarr integration.

> **📝 Write down your API Key!** You'll need it in the next steps.  
> It looks like: `flasharr_359a001fd1604a3f825aa260336b9195`

> **💡 Note:** Flasharr comes with TMDB (The Movie Database) integration built-in — no configuration needed. TMDB provides movie and TV show metadata (titles, posters, year, etc.) for organizing your downloads.

---

## 📺 Step 7 — Add Flasharr to Sonarr (TV Shows)

> **Skip this step** if you don't use Sonarr.

You need to add Flasharr to Sonarr **twice** — once as a search indexer and once as a download client.

### 7.1 — Add Flasharr as a Newznab Indexer (Search)

This lets Sonarr **search** for TV shows through Flasharr.

1. Open Sonarr in your browser (usually `http://localhost:8989`)
2. Go to **Settings** → **Indexers**
3. Click the **+** (Add) button
4. Select **Newznab** from the list

Fill in the fields:

| Field                         | Value                                 |
| ----------------------------- | ------------------------------------- |
| **Name**                      | `Flasharr`                            |
| **Enable RSS Sync**           | `No` (optional, can enable later)     |
| **Enable Automatic Search**   | `Yes` ✅                              |
| **Enable Interactive Search** | `Yes` ✅                              |
| **URL**                       | `http://flasharr:8484/api/newznab`    |
| **API Key**                   | Your Flasharr API key (from Step 6.4) |
| **Categories**                | `5000,5030,5040`                      |

> **🔧 URL Troubleshooting:**
>
> - If Sonarr and Flasharr are on the **same Docker network** → use `http://flasharr:8484/api/newznab`
> - If Sonarr is on a **different machine** → use `http://YOUR_FLASHARR_IP:8484/api/newznab`
> - Example: `http://192.168.1.100:8484/api/newznab`

5. Click **Test** — you should see a green checkmark ✅
6. Click **Save**

### 7.2 — Add Flasharr as a SABnzbd Download Client

This lets Sonarr **send downloads** to Flasharr automatically.

1. In Sonarr, go to **Settings** → **Download Clients**
2. Click the **+** (Add) button
3. Select **SABnzbd** from the list

Fill in the fields:

| Field        | Value                                 |
| ------------ | ------------------------------------- |
| **Name**     | `Flasharr`                            |
| **Enable**   | `Yes` ✅                              |
| **Host**     | `flasharr`                            |
| **Port**     | `8484`                                |
| **API Key**  | Your Flasharr API key (from Step 6.4) |
| **URL Base** | `/sabnzbd`                            |
| **Use SSL**  | `No` ☐                                |
| **Category** | `tv`                                  |

> **🔧 Host Troubleshooting:**
>
> - If Sonarr and Flasharr are on the **same Docker network** → use `flasharr`
> - If Sonarr is on a **different machine** → use your Flasharr machine's IP address
> - Example: `192.168.1.100`

4. Click **Test** — you should see a green checkmark ✅
5. Click **Save**

### 7.3 — Configure Sonarr Root Folder

Make sure Sonarr knows where to find downloaded TV shows:

1. Go to **Settings** → **Media Management**
2. Under **Root Folders**, add: `/data/downloads/tv` (or wherever Flasharr saves TV shows)

> **💡** The root folder should match the path where Flasharr organizes downloaded TV episodes.

---

## 🎬 Step 8 — Add Flasharr to Radarr (Movies)

> **Skip this step** if you don't use Radarr.

The process is identical to Sonarr, with slightly different values.

### 8.1 — Add Flasharr as a Newznab Indexer (Search)

1. Open Radarr in your browser (usually `http://localhost:7878`)
2. Go to **Settings** → **Indexers**
3. Click the **+** (Add) button
4. Select **Newznab** from the list

Fill in the fields:

| Field                         | Value                                 |
| ----------------------------- | ------------------------------------- |
| **Name**                      | `Flasharr`                            |
| **Enable RSS Sync**           | `No` (optional)                       |
| **Enable Automatic Search**   | `Yes` ✅                              |
| **Enable Interactive Search** | `Yes` ✅                              |
| **URL**                       | `http://flasharr:8484/api/newznab`    |
| **API Key**                   | Your Flasharr API key (from Step 6.4) |
| **Categories**                | `2000,2040,2045`                      |

5. Click **Test** → green checkmark ✅
6. Click **Save**

### 8.2 — Add Flasharr as a SABnzbd Download Client

1. In Radarr, go to **Settings** → **Download Clients**
2. Click the **+** (Add) button
3. Select **SABnzbd** from the list

Fill in the fields:

| Field        | Value                                 |
| ------------ | ------------------------------------- |
| **Name**     | `Flasharr`                            |
| **Enable**   | `Yes` ✅                              |
| **Host**     | `flasharr`                            |
| **Port**     | `8484`                                |
| **API Key**  | Your Flasharr API key (from Step 6.4) |
| **URL Base** | `/sabnzbd`                            |
| **Use SSL**  | `No` ☐                                |
| **Category** | `movies`                              |

4. Click **Test** → green checkmark ✅
5. Click **Save**

### 8.3 — Configure Radarr Root Folder

1. Go to **Settings** → **Media Management**
2. Under **Root Folders**, add: `/data/downloads/movies`

---

## ✅ Step 9 — Verify Everything Works

### 9.1 — Test Flasharr is Running

Open your browser and go to:

```
http://localhost:8484/api/health
```

You should see a JSON response like:

```json
{ "status": "ok" }
```

### 9.2 — Test Sonarr Integration

1. Open Sonarr → **Wanted** → **Manual Search** (or add a new TV show)
2. Try searching for a show
3. You should see results from Flasharr
4. Try grabbing an episode — it should appear in Flasharr's download queue

### 9.3 — Test Radarr Integration

1. Open Radarr → **Add New Movie**
2. Search for a movie and add it
3. Try a manual search or click "Search Monitored"
4. Check if results appear from Flasharr
5. Try grabbing a movie — it should appear in Flasharr's download queue

### 9.4 — Check Logs If Something Goes Wrong

```bash
# View Flasharr logs in real-time
docker logs -f flasharr

# View last 50 lines of logs
docker logs --tail 50 flasharr
```

---

## 🔄 Optional — Auto-Update with Watchtower

Watchtower automatically keeps Flasharr up-to-date by checking for new Docker images daily.

### Replace your `docker-compose.yml` with this version:

```yaml
version: "3.8"

services:
  flasharr:
    image: ghcr.io/duytran1406/flasharr:latest
    container_name: flasharr
    restart: unless-stopped
    ports:
      - "8484:8484"
    volumes:
      - ./appData:/appData
      # Uncomment and customize if needed:
      # - /mnt/media/downloads:/appData/downloads
    environment:
      - TZ=Asia/Bangkok
      - RUST_LOG=flasharr=info,tower_http=info
      - FLASHARR_APPDATA_DIR=/appData
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8484/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
    labels:
      - "com.centurylinklabs.watchtower.enable=true"

  watchtower:
    image: containrrr/watchtower:latest
    container_name: flasharr-watchtower
    restart: unless-stopped
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    environment:
      - WATCHTOWER_CLEANUP=true # Remove old images after update
      - WATCHTOWER_LABEL_ENABLE=true # Only update labeled containers
      - WATCHTOWER_INCLUDE_RESTARTING=true
      - WATCHTOWER_SCHEDULE=0 0 4 * * * # Check daily at 4 AM
```

Then restart:

```bash
cd ~/flasharr
docker compose down
docker compose up -d
```

---

## ⬆️ Updating Flasharr

If you're **not** using Watchtower, update manually with:

```bash
cd ~/flasharr

# Pull the latest image
docker compose pull

# Restart with the new image
docker compose up -d
```

Your data and settings are preserved — they live in the `appData` folder.

---

## 🆘 Troubleshooting

### ❌ "Connection refused" when testing in Sonarr/Radarr

**Cause:** Sonarr/Radarr can't reach Flasharr over the network.

**Fix A — Same Docker Compose file:**
If all containers are in the same `docker-compose.yml`, they can talk to each other by container name. Use `flasharr` as the host.

**Fix B — Different Docker Compose files / different machines:**
Use your machine's actual IP address (e.g., `192.168.1.100`) instead of `flasharr`.

**Fix C — Put them on the same Docker network:**

Add a shared network to all your compose services:

```yaml
services:
  flasharr:
    # ... your config ...
    networks:
      - media

  sonarr:
    # ... your config ...
    networks:
      - media

  radarr:
    # ... your config ...
    networks:
      - media

networks:
  media:
    driver: bridge
```

### ❌ "API Key Invalid" in Sonarr/Radarr

1. Open Flasharr → **Settings** → **Services**
2. Copy the API key exactly (no extra spaces)
3. Paste it in Sonarr/Radarr
4. If still failing, regenerate the key in Flasharr and re-enter it

### ❌ Container Won't Start

```bash
# Check what went wrong
docker logs flasharr

# Check if port 8484 is already in use
# Linux/macOS:
lsof -i :8484

# If another service is using the port, change Flasharr's port:
# In docker-compose.yml, change "8484:8484" to "8585:8484"
# Then access Flasharr at http://localhost:8585
```

### ❌ Downloads Not Starting

1. Check that your Fshare credentials are correct in **Settings → Fshare**
2. Verify your Fshare VIP subscription is active
3. Check Flasharr logs for error details:
   ```bash
   docker logs flasharr | grep -i error
   ```

### ❌ Files Downloaded but Not Imported by Sonarr/Radarr

1. **Check permissions:** Sonarr/Radarr must be able to read the download folder
2. **Check root folder:** The root folder in Sonarr/Radarr must match where Flasharr saves files
3. **Check folder structure:** Flasharr creates organized folders like:
   ```
   Movies:  /downloads/movies/Movie Name (Year)/file.mkv
   TV:      /downloads/tv/Show Name/Season 01/file.mkv
   ```

### ❌ Search Returns No Results

1. Try searching with the original English title
2. Check your internet connection
3. Check Flasharr logs for API errors: `docker logs flasharr | grep -i tmdb`

---

## 📋 Useful Commands Cheat Sheet

| Action                       | Command                                       |
| ---------------------------- | --------------------------------------------- |
| **Start Flasharr**           | `docker compose up -d`                        |
| **Stop Flasharr**            | `docker compose down`                         |
| **Restart Flasharr**         | `docker compose restart`                      |
| **View live logs**           | `docker logs -f flasharr`                     |
| **View last 100 log lines**  | `docker logs --tail 100 flasharr`             |
| **Check status**             | `docker compose ps`                           |
| **Update to latest version** | `docker compose pull && docker compose up -d` |
| **Save logs to file**        | `docker logs flasharr > flasharr.log 2>&1`    |
| **Check disk usage**         | `docker system df`                            |
| **Remove old Docker images** | `docker image prune -a`                       |

---

## 📊 Quick Reference Card

| Item                   | Value                                 |
| ---------------------- | ------------------------------------- |
| **Web UI**             | `http://localhost:8484`               |
| **Health Check**       | `http://localhost:8484/api/health`    |
| **Default Port**       | `8484`                                |
| **Config Location**    | `./appData/`                          |
| **Sonarr Newznab URL** | `http://flasharr:8484/api/newznab`    |
| **Sonarr Categories**  | `5000,5030,5040`                      |
| **Radarr Categories**  | `2000,2040,2045`                      |
| **SABnzbd URL Base**   | `/sabnzbd`                            |
| **Docker Image**       | `ghcr.io/duytran1406/flasharr:latest` |

---

## 🎉 You're All Set!

Flasharr is now installed and integrated with your media stack. Here's what happens now:

1. **Search in Sonarr/Radarr** → Flasharr finds content on Fshare
2. **Grab a release** → Flasharr downloads it automatically
3. **Auto-import** → Sonarr/Radarr imports the file into your library

Enjoy your automated media management! ⚡

---

**Need help?**

- 📖 [Full Documentation](https://github.com/duytran1406/flasharr/tree/main/docs)
- 🐛 [Report Issues](https://github.com/duytran1406/flasharr/issues)
- 💬 [Discussions](https://github.com/duytran1406/flasharr/discussions)
