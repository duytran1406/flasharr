#!/bin/bash
# Run URL refresh tests inside Docker container

echo "🧪 Running URL Refresh Tests..."
echo "================================"

# Run tests inside the flasharr container
docker exec flasharr python3 -m pytest /app/tests/unit/test_url_refresh.py -v --tb=short

echo ""
echo "================================"
echo "✅ Test run complete!"
