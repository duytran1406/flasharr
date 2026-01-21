# Newznab API Reference

Flasharr implements the Newznab/Torznab API for integration with Prowlarr and other indexer-compatible applications.

---

## Base URL

```
http://localhost:8484/indexer
```

---

## Authentication

Flasharr's Newznab API does not require authentication. The `apikey` parameter is optional and can be any value.

---

## Endpoints

### 1. Capabilities (`?t=caps`)

Returns the indexer's capabilities in XML format.

**Request:**
```http
GET /indexer/api?t=caps
```

**Response:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<caps>
  <server title="Flasharr" version="0.0.3-beta"/>
  <limits max="100" default="100"/>
  <searching>
    <search available="yes" supportedParams="q"/>
    <tv-search available="yes" supportedParams="q,season,ep"/>
    <movie-search available="yes" supportedParams="q,imdbid"/>
  </searching>
  <categories>
    <category id="2000" name="Movies">
      <subcat id="2010" name="Foreign"/>
      <subcat id="2040" name="HD"/>
      <subcat id="2050" name="3D"/>
    </category>
    <category id="5000" name="TV">
      <subcat id="5030" name="Foreign"/>
      <subcat id="5040" name="HD"/>
    </category>
  </categories>
</caps>
```

---

### 2. Search (`?t=search`)

Search for files on Fshare.

**Request:**
```http
GET /indexer/api?t=search&q={query}&limit={limit}
```

**Parameters:**
| Parameter | Required | Description |
|-----------|----------|-------------|
| `t` | Yes | Must be `search` |
| `q` | Yes | Search query |
| `limit` | No | Max results (default: 100) |
| `cat` | No | Category filter (2000=Movies, 5000=TV) |
| `apikey` | No | API key (optional) |

**Example:**
```bash
curl "http://localhost:8484/indexer/api?t=search&q=Inception&limit=10"
```

**Response:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:newznab="http://www.newznab.com/DTD/2010/feeds/attributes/">
  <channel>
    <title>Flasharr</title>
    <description>Fshare Indexer</description>
    <item>
      <title>Inception (2010) 1080p BluRay x264</title>
      <guid>https://fshare.vn/file/ABC123</guid>
      <link>http://localhost:8484/indexer/nzb/ABC123</link>
      <pubDate>Mon, 10 Jan 2026 00:00:00 +0000</pubDate>
      <category>2000</category>
      <enclosure url="http://localhost:8484/indexer/nzb/ABC123" length="2147483648" type="application/x-nzb"/>
      <newznab:attr name="category" value="2000"/>
      <newznab:attr name="size" value="2147483648"/>
      <newznab:attr name="guid" value="ABC123"/>
    </item>
  </channel>
</rss>
```

---

### 3. TV Search (`?t=tvsearch`)

Search for TV shows with season/episode filtering.

**Request:**
```http
GET /indexer/api?t=tvsearch&q={query}&season={season}&ep={episode}
```

**Parameters:**
| Parameter | Required | Description |
|-----------|----------|-------------|
| `t` | Yes | Must be `tvsearch` |
| `q` | Yes | TV show name |
| `season` | No | Season number |
| `ep` | No | Episode number |
| `limit` | No | Max results (default: 100) |

**Example:**
```bash
curl "http://localhost:8484/indexer/api?t=tvsearch&q=Breaking+Bad&season=1&ep=1"
```

---

### 4. Movie Search (`?t=movie`)

Search for movies with IMDb ID support.

**Request:**
```http
GET /indexer/api?t=movie&q={query}&imdbid={imdbid}
```

**Parameters:**
| Parameter | Required | Description |
|-----------|----------|-------------|
| `t` | Yes | Must be `movie` |
| `q` | No | Movie title |
| `imdbid` | No | IMDb ID (e.g., tt1375666) |
| `limit` | No | Max results (default: 100) |

**Example:**
```bash
curl "http://localhost:8484/indexer/api?t=movie&imdbid=tt1375666"
```

---

### 5. Get NZB (`/nzb/{guid}`)

Download the NZB file for a specific result.

**Request:**
```http
GET /indexer/nzb/{guid}
```

**Example:**
```bash
curl "http://localhost:8484/indexer/nzb/ABC123" -o file.nzb
```

**Response:**
Returns an NZB file containing the Fshare download URL.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">
  <file>
    <segments>
      <segment>https://fshare.vn/file/ABC123</segment>
    </segments>
  </file>
</nzb>
```

---

## Newznab Attributes

Flasharr includes the following Newznab attributes in search results:

| Attribute | Description |
|-----------|-------------|
| `category` | Category ID (2000=Movies, 5000=TV) |
| `size` | File size in bytes |
| `guid` | Unique identifier (Fshare file code) |

---

## Categories

| ID | Name | Description |
|----|------|-------------|
| 2000 | Movies | All movies |
| 2010 | Movies/Foreign | Foreign language movies |
| 2040 | Movies/HD | HD movies (720p, 1080p, 4K) |
| 2050 | Movies/3D | 3D movies |
| 5000 | TV | All TV shows |
| 5030 | TV/Foreign | Foreign language TV |
| 5040 | TV/HD | HD TV shows |

---

## Error Responses

### Invalid Request

```xml
<?xml version="1.0" encoding="UTF-8"?>
<error code="200" description="Missing parameter: q"/>
```

### No Results

```xml
<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Flasharr</title>
    <description>Fshare Indexer</description>
  </channel>
</rss>
```

---

## Integration Examples

### Prowlarr Configuration

1. **Settings** → **Indexers** → **Add Indexer**
2. Select **Newznab**
3. Configure:
   ```
   Name: Fshare (Flasharr)
   URL: http://flasharr:8484/indexer
   API Path: /api
   API Key: (any value or leave empty)
   Categories: 2000, 5000
   ```

### Manual cURL Test

```bash
# Test capabilities
curl "http://localhost:8484/indexer/api?t=caps"

# Search for a movie
curl "http://localhost:8484/indexer/api?t=search&q=Interstellar"

# Search for TV show
curl "http://localhost:8484/indexer/api?t=tvsearch&q=The+Office&season=1&ep=1"

# Download NZB
curl "http://localhost:8484/indexer/nzb/ABC123" -o test.nzb
```

---

## Notes

> [!NOTE]
> Flasharr searches TimFshare.com (a Fshare search engine) for results, not Fshare.vn directly.

> [!TIP]
> Use specific search terms for better results. Generic queries may return many irrelevant files.

> [!WARNING]
> The `imdbid` parameter is supported but may not always return accurate results due to Fshare's metadata limitations.

---

## Next Steps

- [SABnzbd API](sabnzbd-api.md) - Download client integration
- [Engine API](engine-api.md) - Direct download management
- [Integration Flow](../architecture/integration-flow.md) - How it all works together
