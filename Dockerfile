FROM python:3.11-slim

# Set working directory
WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy requirements first for better caching
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application code
COPY src/ ./src/
COPY run.py .
COPY VERSION .
COPY .env.example .env.example

# Create download directory
RUN mkdir -p /downloads

# Set environment variables
ENV PYTHONUNBUFFERED=1
ENV DOWNLOAD_DIR=/downloads

# Expose port
EXPOSE 8484

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD python3 -c "import requests; requests.get('http://localhost:8484/health', timeout=5)" || exit 1

# Run application
CMD ["python3", "run.py"]
