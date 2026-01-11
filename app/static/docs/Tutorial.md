# Fshare Nexus Tutorial

Welcome to Fshare Nexus! This tool helps you bridge Fshare.vn downloads with your favorite media managers like Sonarr and Radarr.

## Features
- **Direct Search**: Search Fshare.vn directly from the dashboard.
- **Easy Downloading**: Add links manually or automatically via Sonarr/Radarr.
- **Categorization**: Downloads are automatically tagged based on their source.
- **Real-time Monitoring**: Track download speeds and progress in real-time.

## Installation & Setup

### 1. Account Configuration
Ensure your Fshare.vn and pyLoad credentials are correctly set in the `.env` file within the application directory.

### 2. Storage Configuration (ZFS)
Inside LXC 112, your ZFS volumes are mounted at `/mnt/appdata` and `/data`.

To ensure your downloads are saved correctly to your ZFS pool:
1.  **pyLoad Settings**: You must manually update this in the pyLoad Web UI:
    - Access pyLoad at `http://[YOUR_IP]:8100`.
    - Go to **Config** -> **General** -> **Download Directory**.
    - Set the value to exactly: `/fshare-downloader`.
    - Click **Save Changes** at the bottom.

2.  **Path Mapping**: This container path is mapped to the LXC directory `/data/fshare-downloader`, which is the actual ZFS volume `/bulk-storage/data/fshare-downloader` on your host.

### 3. Dashboard Usage
Use the search bar at the top to find movies or series. Click "Download" to add them to your queue.

### 3. Integrating with Sonarr/Radarr
Fshare Nexus emulates a SABnzbd API. In your *arr application:
- Add a new **Download Client** of type **SABnzbd**.
- Set the **Host** to the IP of your Fshare Nexus instance.
- Set the **Port** to `8484`.
- Use any API Key (it's currently ignored but required by the app).
- Set the **Category** to `sonarr` or `radarr` respectively.

## Downloads Tab
The Downloads tab gives you advanced control over your queue:
- **Search**: Filter your local queue by filename.
- **Global Actions**: Start, pause, or stop all downloads at once.
- **Add Link**: Manually paste any Fshare.vn file link to start a download.

## Support
For more information, please check the project repository or contact the administrator.
