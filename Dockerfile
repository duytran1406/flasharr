FROM python:3.11-slim

LABEL maintainer="Fshare-Arr Bridge"
LABEL description="Integration bridge for Fshare.vn with Sonarr/Radarr"

# Set working directory
WORKDIR /app

# Install dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application
COPY app/ ./app/
COPY src/ ./src/
COPY VERSION .

# Set PYTHONPATH to include src directory
ENV PYTHONPATH="/app/src:${PYTHONPATH}"

# Create non-root user
RUN useradd -m -u 1000 bridge && \
    chown -R bridge:bridge /app

USER bridge

# Expose port
EXPOSE 8484

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python -c "import requests; requests.get('http://localhost:8484/health')"

# Run application using the new package structure
CMD ["gunicorn", "--bind", "0.0.0.0:8484", "--workers", "2", "--timeout", "120", "fshare_bridge.web.app:create_app()"]
