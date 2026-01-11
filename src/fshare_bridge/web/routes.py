"""
Main Routes Blueprint

Handles page rendering for the web UI.
"""

import logging
import markdown
from pathlib import Path
from flask import Blueprint, render_template, send_from_directory

logger = logging.getLogger(__name__)

main_bp = Blueprint("main", __name__)


@main_bp.route("/")
def index():
    """Render the dashboard page."""
    return render_template("index.html")


@main_bp.route("/downloads")
def downloads_page():
    """Render the downloads page."""
    return render_template("downloads.html")


@main_bp.route("/search")
def search_page():
    """Render the search page."""
    return render_template("search.html")


@main_bp.route("/settings")
def settings_page():
    """Render the settings page."""
    return render_template("settings.html")


@main_bp.route("/about")
def about_page():
    """Render the about/documentation page."""
    readme_content = ""
    
    # Try to load README.md
    readme_paths = [
        Path("/app/README.md"),
        Path(__file__).parent.parent.parent.parent.parent / "README.md",
    ]
    
    for readme_path in readme_paths:
        if readme_path.exists():
            try:
                with open(readme_path, "r", encoding="utf-8") as f:
                    readme_content = f.read()
                break
            except Exception as e:
                logger.error(f"Error reading README: {e}")
    
    # Convert markdown to HTML
    if readme_content:
        readme_html = markdown.markdown(
            readme_content,
            extensions=["fenced_code", "tables", "toc"],
        )
    else:
        readme_html = "<p>Documentation not found.</p>"
    
    return render_template("about.html", content=readme_html)

# Legacy Redirect/Alias if needed, or just remove tutorial route
@main_bp.route("/tutorial")
def tutorial_redirect():
     return about_page()


@main_bp.route("/health")
def health_check():
    """Health check endpoint for container orchestration."""
    return {"status": "healthy"}, 200


@main_bp.route("/favicon.ico")
def favicon():
    """Serve favicon."""
    return send_from_directory(
        main_bp.static_folder or "static",
        "favicon.ico",
        mimetype="image/x-icon",
    )
