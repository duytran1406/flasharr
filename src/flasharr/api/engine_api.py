"""
API Endpoints for Enhanced Download Engine

Provides REST API for controlling beta features:
- Speed limiting
- Priority management
- Account management
- Link checking
- Engine statistics
"""

from flask import Blueprint, jsonify, request
import logging

from ..core.priority_queue import Priority

logger = logging.getLogger(__name__)

# Blueprint for engine control API
engine_api = Blueprint('engine_api', __name__, url_prefix='/api/engine')


def init_engine_api(app, engine):
    """
    Initialize engine API with Flask app.
    
    Args:
        app: Flask application
        engine: EnhancedDownloadEngine instance
    """
    # Store engine reference
    engine_api.engine = engine
    
    # Register blueprint
    app.register_blueprint(engine_api)
    
    logger.info("Engine API initialized")


@engine_api.route('/stats', methods=['GET'])
def get_stats():
    """Get comprehensive engine statistics."""
    try:
        stats = engine_api.engine.get_engine_stats()
        return jsonify(stats), 200
    except Exception as e:
        logger.error(f"Failed to get stats: {e}")
        return jsonify({"error": str(e)}), 500


@engine_api.route('/speed-limit', methods=['GET', 'POST'])
def speed_limit():
    """Get or set global speed limit."""
    if request.method == 'GET':
        # Get current limit
        stats = engine_api.engine.rate_limiter.get_stats()
        return jsonify(stats), 200
    
    else:  # POST
        try:
            data = request.get_json()
            limit_mbps = data.get('limit_mbps')
            
            if limit_mbps is None or limit_mbps == 0:
                # Disable limit
                engine_api.engine.set_global_speed_limit(None)
                return jsonify({"message": "Speed limit disabled"}), 200
            else:
                # Set limit (convert MB/s to bytes/s)
                bytes_per_sec = int(limit_mbps * 1024 * 1024)
                engine_api.engine.set_global_speed_limit(bytes_per_sec)
                return jsonify({
                    "message": f"Speed limit set to {limit_mbps} MB/s",
                    "bytes_per_sec": bytes_per_sec
                }), 200
        
        except Exception as e:
            logger.error(f"Failed to set speed limit: {e}")
            return jsonify({"error": str(e)}), 400


@engine_api.route('/priority/<task_id>', methods=['POST'])
def set_priority(task_id):
    """Set priority for a task."""
    try:
        data = request.get_json()
        priority_name = data.get('priority', 'NORMAL').upper()
        
        # Parse priority
        try:
            priority = Priority[priority_name]
        except KeyError:
            return jsonify({
                "error": f"Invalid priority. Must be one of: {[p.name for p in Priority]}"
            }), 400
        
        # Set priority
        success = engine_api.engine.set_task_priority(task_id, priority)
        
        if success:
            return jsonify({
                "message": f"Priority set to {priority.name}",
                "task_id": task_id,
                "priority": priority.name
            }), 200
        else:
            return jsonify({"error": "Task not found"}), 404
    
    except Exception as e:
        logger.error(f"Failed to set priority: {e}")
        return jsonify({"error": str(e)}), 500


@engine_api.route('/accounts', methods=['GET'])
def get_accounts():
    """Get account balancer statistics."""
    try:
        if not engine_api.engine.account_balancer:
            return jsonify({"error": "Multi-account support not enabled"}), 400
        
        stats = engine_api.engine.account_balancer.get_stats()
        return jsonify(stats), 200
    
    except Exception as e:
        logger.error(f"Failed to get account stats: {e}")
        return jsonify({"error": str(e)}), 500


@engine_api.route('/accounts/<email>/reset', methods=['POST'])
def reset_account(email):
    """Manually reset account status."""
    try:
        if not engine_api.engine.account_balancer:
            return jsonify({"error": "Multi-account support not enabled"}), 400
        
        success = engine_api.engine.account_balancer.reset_account(email)
        
        if success:
            return jsonify({
                "message": f"Account {email} reset successfully",
                "email": email
            }), 200
        else:
            return jsonify({"error": "Account not found"}), 404
    
    except Exception as e:
        logger.error(f"Failed to reset account: {e}")
        return jsonify({"error": str(e)}), 500


@engine_api.route('/link-check', methods=['POST'])
def check_link():
    """Check link availability."""
    try:
        data = request.get_json()
        url = data.get('url')
        
        if not url:
            return jsonify({"error": "URL required"}), 400
        
        # Perform check
        import asyncio
        result = asyncio.run(
            engine_api.engine.link_checker.check_link(
                url,
                session=engine_api.engine._session,
                force_recheck=data.get('force_recheck', False)
            )
        )
        
        return jsonify(result.to_dict()), 200
    
    except Exception as e:
        logger.error(f"Failed to check link: {e}")
        return jsonify({"error": str(e)}), 500


@engine_api.route('/link-check/cache', methods=['DELETE'])
def clear_link_cache():
    """Clear link check cache."""
    try:
        data = request.get_json() or {}
        url = data.get('url')
        
        engine_api.engine.link_checker.clear_cache(url)
        
        if url:
            return jsonify({"message": f"Cache cleared for {url}"}), 200
        else:
            return jsonify({"message": "All cache cleared"}), 200
    
    except Exception as e:
        logger.error(f"Failed to clear cache: {e}")
        return jsonify({"error": str(e)}), 500


@engine_api.route('/config', methods=['GET', 'POST'])
def engine_config():
    """Get or update engine configuration."""
    if request.method == 'GET':
        return jsonify({
            "max_concurrent": engine_api.engine.max_concurrent,
            "segments_per_download": engine_api.engine.segments_per_download,
            "enable_dynamic_scaling": engine_api.engine.enable_dynamic_scaling,
            "min_segments": engine_api.engine.min_segments,
            "max_segments": engine_api.engine.max_segments,
        }), 200
    
    else:  # POST
        try:
            data = request.get_json()
            
            if 'enable_dynamic_scaling' in data:
                engine_api.engine.enable_dynamic_scaling = bool(data['enable_dynamic_scaling'])
            
            if 'min_segments' in data:
                engine_api.engine.min_segments = int(data['min_segments'])
            
            if 'max_segments' in data:
                engine_api.engine.max_segments = int(data['max_segments'])
            
            return jsonify({
                "message": "Configuration updated",
                "config": {
                    "enable_dynamic_scaling": engine_api.engine.enable_dynamic_scaling,
                    "min_segments": engine_api.engine.min_segments,
                    "max_segments": engine_api.engine.max_segments,
                }
            }), 200
        
        except Exception as e:
            logger.error(f"Failed to update config: {e}")
            return jsonify({"error": str(e)}), 400
