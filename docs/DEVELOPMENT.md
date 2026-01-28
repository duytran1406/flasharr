# Development Guide

## Prerequisites

- **Rust** 1.75+ (for backend)
- **Node.js** 20+ (for frontend)
- **pnpm** or **npm**

## Quick Start

### Development Mode

```bash
# Start both frontend and backend with hot reload
./scripts/debug/dev.sh
```

- Frontend: http://localhost:5173
- Backend: http://localhost:8484

### Manual Start

**Backend:**

```bash
cd backend
cargo run
```

**Frontend:**

```bash
cd frontend
npm install
npm run dev
```

---

## Project Structure

```
backend/
├── src/
│   ├── main.rs          # Entry point, router setup
│   ├── api/             # REST endpoints
│   ├── downloader/      # Download engine
│   ├── hosts/           # Fshare integration
│   ├── db/              # SQLite operations
│   └── utils/           # Parser, tokenizer
└── Cargo.toml

frontend/
├── src/
│   ├── routes/          # SvelteKit pages
│   ├── lib/
│   │   ├── components/  # UI components
│   │   ├── stores/      # State management
│   │   └── services/    # API clients
│   └── app.html
└── package.json
```

---

## Configuration

### Backend (`config.toml`)

```toml
[server]
port = 8484

[downloads]
max_concurrent = 3
directory = "/downloads"

[sonarr]
enabled = false
url = "http://localhost:8989"
api_key = ""

[radarr]
enabled = false
url = "http://localhost:7878"
api_key = ""
```

### Frontend (`vite.config.ts`)

```typescript
proxy: {
  '/api': 'http://localhost:8484'
}
```

---

## Docker

### Build

```bash
docker build -t flasharr .
```

### Run

```bash
docker-compose up -d
```

---

## Common Tasks

### Add New API Endpoint

1. Create handler in `backend/src/api/`
2. Add route in `main.rs`
3. Create frontend service in `frontend/src/lib/services/`

### Add New Component

1. Create in `frontend/src/lib/components/`
2. Export from `components/index.ts`
3. Import where needed

### Database Migration

SQLite schema is auto-created in `backend/src/db/mod.rs`
