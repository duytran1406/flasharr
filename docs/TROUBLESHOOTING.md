# Troubleshooting Guide

## Common Issues

### Installation Issues

#### Docker Not Found

**Symptom**: `docker: command not found`

**Solution**:

```bash
# Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

#### Permission Denied

**Symptom**: `permission denied while trying to connect to the Docker daemon socket`

**Solution**:

```bash
sudo usermod -aG docker $USER
newgrp docker
```

### Container Issues

#### Container Won't Start

**Check logs**:

```bash
docker compose logs flasharr
```

**Common causes**:

1. Port 8484 already in use

   ```bash
   # Check what's using the port
   sudo lsof -i :8484

   # Change port in docker-compose.yml
   ports:
     - "8485:8484"  # Use different host port
   ```

2. Permission issues with appData
   ```bash
   sudo chown -R 1000:1000 ./appData
   chmod -R 755 ./appData
   ```

#### Container Keeps Restarting

**Check health status**:

```bash
docker inspect flasharr | grep -A 10 Health
```

**View detailed logs**:

```bash
docker compose logs -f --tail=100 flasharr
```

### Application Issues

#### Can't Access Web UI

1. **Check if container is running**:

   ```bash
   docker ps | grep flasharr
   ```

2. **Verify port mapping**:

   ```bash
   docker port flasharr
   ```

3. **Check firewall**:

   ```bash
   sudo ufw allow 8484/tcp
   ```

4. **Test locally**:
   ```bash
   curl http://localhost:8484/health
   ```

#### FShare Login Fails

**Symptom**: "Failed to login to FShare" in logs

**Solutions**:

1. Verify credentials in Settings → Services
2. Check FShare account status
3. Clear session and retry:
   ```bash
   docker compose restart flasharr
   ```

#### Downloads Not Starting

**Check**:

1. Download directory permissions
2. Available disk space: `df -h`
3. FShare account status (VIP required for some files)
4. Application logs for specific errors

#### WebSocket Connection Failed

**Symptom**: Real-time updates not working

**Solutions**:

1. Check reverse proxy WebSocket configuration
2. Verify no firewall blocking WebSocket
3. Clear browser cache and reload

### Performance Issues

#### Slow Download Speeds

1. **Check network**:

   ```bash
   docker exec flasharr ping -c 4 8.8.8.8
   ```

2. **Monitor resource usage**:

   ```bash
   docker stats flasharr
   ```

3. **Increase resource limits** in docker-compose.yml

#### High Memory Usage

**Check current usage**:

```bash
docker stats flasharr --no-stream
```

**Set memory limit**:

```yaml
deploy:
  resources:
    limits:
      memory: 512M
```

### Database Issues

#### Database Locked

**Symptom**: "database is locked" errors

**Solution**:

```bash
docker compose restart flasharr
```

#### Corrupted Database

**Restore from backup**:

```bash
docker compose down
cp appData/data/flasharr.db.backup appData/data/flasharr.db
docker compose up -d
```

**Start fresh** (loses all data):

```bash
docker compose down
rm appData/data/flasharr.db
docker compose up -d
```

### Integration Issues

#### Sonarr/Radarr Can't Connect

1. **Verify API key** in Settings → Indexer
2. **Check network connectivity**:
   - If using Docker networks, ensure containers can communicate
   - Use container name or host IP, not `localhost`

3. **Test indexer endpoint**:
   ```bash
   curl http://localhost:8484/api/indexer/caps?apikey=YOUR_API_KEY
   ```

#### Indexer Returns No Results

1. Check FShare credentials are configured
2. Verify search query format
3. Check application logs for errors

## Diagnostic Commands

### View All Logs

```bash
docker compose logs -f flasharr
```

### Check Container Health

```bash
docker inspect flasharr --format='{{.State.Health.Status}}'
```

### View Resource Usage

```bash
docker stats flasharr
```

### Test API Endpoint

```bash
curl -v http://localhost:8484/health
```

### Export Logs to File

```bash
docker compose logs flasharr > flasharr-logs.txt
```

## Reset to Default

### Soft Reset (Keep Data)

```bash
docker compose restart flasharr
```

### Hard Reset (Lose All Data)

```bash
docker compose down
rm -rf appData
docker compose up -d
```

### Clean Reinstall

```bash
docker compose down
docker rmi flasharr:latest
rm -rf appData
docker compose up -d
```

## Getting Help

If you're still experiencing issues:

1. **Check existing issues**: [GitHub Issues](https://github.com/duytran1406/flasharr/issues)
2. **Create a new issue** with:
   - Docker version: `docker --version`
   - Docker Compose version: `docker compose version`
   - OS and version
   - Full error logs
   - Steps to reproduce

3. **Join our community**: [Discord](https://discord.gg/flasharr-community)

## Debug Mode

Enable verbose logging:

```yaml
environment:
  - RUST_LOG=flasharr=debug,tower_http=debug
```

Then restart:

```bash
docker compose up -d
docker compose logs -f flasharr
```
