"""
Download Queue Manager

Persistent queue management with SQLite storage.
"""

import sqlite3
import logging
from pathlib import Path
from typing import List, Optional, Dict, Any
from datetime import datetime
from contextlib import contextmanager

from .engine import DownloadTask, DownloadState, DownloadProgress

logger = logging.getLogger(__name__)


class DownloadQueue:
    """
    Persistent download queue with SQLite backend.
    
    Provides:
    - Persistent storage of download tasks
    - Queue prioritization
    - History tracking
    - Statistics
    
    Example:
        >>> queue = DownloadQueue("/app/data/downloads.db")
        >>> queue.add_task(task)
        >>> pending = queue.get_pending_tasks()
    """
    
    SCHEMA = """
    CREATE TABLE IF NOT EXISTS downloads (
        id TEXT PRIMARY KEY,
        url TEXT NOT NULL,
        filename TEXT NOT NULL,
        destination TEXT NOT NULL,
        state TEXT DEFAULT 'Queued',
        downloaded_bytes INTEGER DEFAULT 0,
        total_bytes INTEGER DEFAULT 0,
        speed REAL DEFAULT 0,
        error_message TEXT,
        category TEXT DEFAULT 'Uncategorized',
        package_name TEXT,
        priority INTEGER DEFAULT 0,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        started_at TIMESTAMP,
        completed_at TIMESTAMP,
        wait_until TIMESTAMP,
        retry_count INTEGER DEFAULT 0,
        plugin_name TEXT
    );
    
    CREATE INDEX IF NOT EXISTS idx_state ON downloads(state);
    CREATE INDEX IF NOT EXISTS idx_category ON downloads(category);
    CREATE INDEX IF NOT EXISTS idx_created ON downloads(created_at);
    
    """
    
    def __init__(self, db_path: str = "/app/data/downloads.db"):
        """
        Initialize the queue manager.
        
        Args:
            db_path: Path to SQLite database file
        """
        self.db_path = Path(db_path)
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        
        self._init_db()
    
    def _init_db(self) -> None:
        """Initialize the database schema."""
        with self._get_connection() as conn:
            conn.executescript(self.SCHEMA)
            
            # Migration: Add new columns if they don't exist
            try:
                cursor = conn.cursor()
                # Check for wait_until
                try:
                    cursor.execute("SELECT wait_until FROM downloads LIMIT 1")
                except sqlite3.OperationalError:
                    cursor.execute("ALTER TABLE downloads ADD COLUMN wait_until TIMESTAMP")
                    cursor.execute("ALTER TABLE downloads ADD COLUMN retry_count INTEGER DEFAULT 0")
                    cursor.execute("ALTER TABLE downloads ADD COLUMN plugin_name TEXT")
                    logger.info("Migrated database schema: Added extended state columns")
            except Exception as e:
                logger.error(f"Migration failed: {e}")
    
    
    @contextmanager
    def _get_connection(self):
        """Get a database connection context manager."""
        conn = sqlite3.connect(str(self.db_path))
        conn.row_factory = sqlite3.Row
        try:
            yield conn
            conn.commit()
        finally:
            conn.close()
    
    def add_task(self, task: DownloadTask) -> bool:
        """
        Add a task to the queue.
        
        Args:
            task: DownloadTask to add
            
        Returns:
            True if added successfully
        """
        try:
            with self._get_connection() as conn:
                conn.execute("""
                    INSERT INTO downloads (
                        id, url, filename, destination, state,
                        downloaded_bytes, total_bytes, category, package_name,
                        wait_until, retry_count, plugin_name
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """, (
                    task.id,
                    task.url,
                    task.filename,
                    str(task.destination),
                    task.state.value,
                    task.progress.downloaded_bytes,
                    task.progress.total_bytes,
                    task.category,
                    task.package_name,
                    task.wait_until.isoformat() if task.wait_until else None,
                    task.retry_count,
                    task.plugin_name,
                ))
            return True
        except Exception as e:
            logger.error(f"Failed to add task: {e}")
            return False
    
    def update_task(self, task: DownloadTask) -> bool:
        """
        Update a task in the queue.
        
        Args:
            task: DownloadTask to update
            
        Returns:
            True if updated successfully
        """
        try:
            with self._get_connection() as conn:
                conn.execute("""
                    UPDATE downloads SET
                        state = ?,
                        downloaded_bytes = ?,
                        total_bytes = ?,
                        speed = ?,
                        error_message = ?,
                        started_at = ?,
                        completed_at = ?,
                        wait_until = ?,
                        retry_count = ?
                    WHERE id = ?
                """, (
                    task.state.value,
                    task.progress.downloaded_bytes,
                    task.progress.total_bytes,
                    task.progress.speed_bytes_per_sec,
                    task.error_message,
                    task.started_at.isoformat() if task.started_at else None,
                    task.completed_at.isoformat() if task.completed_at else None,
                    task.wait_until.isoformat() if task.wait_until else None,
                    task.retry_count,
                    task.id,
                ))
            return True
        except Exception as e:
            logger.error(f"Failed to update task: {e}")
            return False
    
    def get_task(self, task_id: str) -> Optional[Dict[str, Any]]:
        """Get a task by ID."""
        with self._get_connection() as conn:
            row = conn.execute(
                "SELECT * FROM downloads WHERE id = ?",
                (task_id,)
            ).fetchone()
            
            return dict(row) if row else None
    
    def get_pending_tasks(self, limit: int = 100) -> List[Dict[str, Any]]:
        """Get tasks waiting to be processed."""
        with self._get_connection() as conn:
            rows = conn.execute("""
                SELECT * FROM downloads
                WHERE state IN ('Queued', 'Paused')
                ORDER BY priority DESC, created_at ASC
                LIMIT ?
            """, (limit,)).fetchall()
            
            return [dict(row) for row in rows]
    
    def get_active_tasks(self) -> List[Dict[str, Any]]:
        """Get currently downloading tasks."""
        with self._get_connection() as conn:
            rows = conn.execute("""
                SELECT * FROM downloads
                WHERE state IN ('Starting', 'Downloading')
                ORDER BY started_at DESC
            """).fetchall()
            
            return [dict(row) for row in rows]
    
    def get_history(self, limit: int = 50) -> List[Dict[str, Any]]:
        """Get completed/failed tasks."""
        with self._get_connection() as conn:
            rows = conn.execute("""
                SELECT * FROM downloads
                WHERE state IN ('Completed', 'Failed', 'Cancelled')
                ORDER BY completed_at DESC
                LIMIT ?
            """, (limit,)).fetchall()
            
            return [dict(row) for row in rows]
    
    def delete_task(self, task_id: str) -> bool:
        """Delete a task from the queue."""
        try:
            with self._get_connection() as conn:
                conn.execute("DELETE FROM downloads WHERE id = ?", (task_id,))
            return True
        except Exception as e:
            logger.error(f"Failed to delete task: {e}")
            return False
    
    def clear_history(self) -> int:
        """Clear completed/failed tasks. Returns count deleted."""
        with self._get_connection() as conn:
            cursor = conn.execute("""
                DELETE FROM downloads
                WHERE state IN ('Completed', 'Failed', 'Cancelled')
            """)
            return cursor.rowcount
    
    def get_statistics(self) -> Dict[str, Any]:
        """Get queue statistics."""
        with self._get_connection() as conn:
            row = conn.execute("""
                SELECT
                    COUNT(*) as total,
                    SUM(CASE WHEN state = 'Queued' THEN 1 ELSE 0 END) as queued,
                    SUM(CASE WHEN state = 'Downloading' THEN 1 ELSE 0 END) as downloading,
                    SUM(CASE WHEN state = 'Paused' THEN 1 ELSE 0 END) as paused,
                    SUM(CASE WHEN state = 'Completed' THEN 1 ELSE 0 END) as completed,
                    SUM(CASE WHEN state = 'Failed' THEN 1 ELSE 0 END) as failed,
                    SUM(total_bytes) as total_bytes,
                    SUM(downloaded_bytes) as downloaded_bytes
                FROM downloads
            """).fetchone()
            
            return {
                "total": row["total"] or 0,
                "queued": row["queued"] or 0,
                "downloading": row["downloading"] or 0,
                "paused": row["paused"] or 0,
                "completed": row["completed"] or 0,
                "failed": row["failed"] or 0,
                "total_bytes": row["total_bytes"] or 0,
                "downloaded_bytes": row["downloaded_bytes"] or 0,
            }
