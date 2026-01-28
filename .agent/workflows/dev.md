---
description: Start local development environment
---

# Local Development

Start the local development stack using Docker Compose.

## Steps

1. **Start Services**
   Run the containers in detached mode.

   // turbo

   ```bash
   docker compose up -d
   ```

2. **Check Logs**
   Follow the logs to ensure everything starts correctly.

   ```bash
   docker compose logs -f
   ```

3. **Access Local App**
   Open your browser at: [http://localhost:8484](http://localhost:8484)
