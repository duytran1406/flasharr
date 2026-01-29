# Flasharr Quick Setup Guide

## 🚀 5-Minute Setup for Sonarr/Radarr

---

## Step 1: Get Your API Key

```bash
# Default API key
flasharr-default-key
```

Or check Settings → Indexer tab in Flasharr UI.

---

## Step 2: Add to Sonarr

### Indexer (Settings → Indexers → + → Newznab)

```
Name:     Flasharr
URL:      http://YOUR_IP:8484/newznab
API Path: /api
API Key:  flasharr-default-key
```

### Download Client (Settings → Download Clients → + → SABnzbd)

```
Name:     Flasharr
Host:     YOUR_IP
Port:     8484
URL Base: /sabnzbd
API Key:  flasharr-default-key
```

---

## Step 3: Add to Radarr

Same as Sonarr! Just use Movie categories instead of TV.

---

## Step 4: Test

1. Search for any series/movie
2. Click "Manual Search"
3. See Vietnamese releases from Fshare! 🎉

---

## Common URLs

| Service              | URL                           |
| -------------------- | ----------------------------- |
| **Flasharr UI**      | `http://YOUR_IP:8484`         |
| **Newznab Endpoint** | `http://YOUR_IP:8484/newznab` |
| **SABnzbd Endpoint** | `http://YOUR_IP:8484/sabnzbd` |
| **Health Check**     | `http://YOUR_IP:8484/health`  |

---

## Troubleshooting

**"Invalid API Key" but test passes?**
→ Ignore it, save anyway. It's a Sonarr UI quirk.

**No results?**
→ Check Fshare credentials in Flasharr Settings.

**Downloads not importing?**
→ Configure Remote Path Mappings (see full guide).

---

**Full Documentation:** See `ARR_INTEGRATION.md`
