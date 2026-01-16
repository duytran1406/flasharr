"""
UI Routes (AIOHTTP)

Frontend page rendering using Jinja2 manually.
"""

import logging
import os
from pathlib import Path
from aiohttp import web
from jinja2 import Environment, FileSystemLoader, select_autoescape

logger = logging.getLogger(__name__)

routes = web.RouteTableDef()
_jinja_env = None

def setup_jinja(app: web.Application, template_path: str):
    """Initialize Jinja2 environment."""
    global _jinja_env
    _jinja_env = Environment(
        loader=FileSystemLoader(template_path),
        autoescape=select_autoescape(['html', 'xml'])
    )
    # Add version global
    try:
        version = (Path("VERSION").read_text().strip()) if Path("VERSION").exists() else "beta"
    except: version = "beta"
    _jinja_env.globals['version'] = version
    
    # Add url_for global
    def url_for(endpoint: str, **values) -> str:
        if endpoint == 'static' and 'filename' in values:
            return f"/static/{values['filename']}"
        # Fallback for other routes if needed (though templates mostly use static)
        return "/"

    _jinja_env.globals['url_for'] = url_for
    
    # Add now global for cache busting
    import time
    _jinja_env.globals['now'] = lambda: int(time.time())

def render_template(request: web.Request, template_name: str, **kwargs) -> web.Response:
    """Render a Jinja2 template to Response."""
    if not _jinja_env:
        return web.Response(text="Template engine not initialized", status=500)
    
    try:
        template = _jinja_env.get_template(template_name)
        html = template.render(request=request, **kwargs)
        return web.Response(text=html, content_type='text/html')
    except Exception as e:
        logger.error(f"Template rendering error for {template_name}: {e}")
        return web.Response(text=f"Template Error: {e}", status=500)

@routes.get("/")
async def index(request: web.Request) -> web.Response:
    # Promote V2 as default
    return render_template(request, "base_v2.html")

@routes.get("/v1")
async def index_legacy(request: web.Request) -> web.Response:
    return render_template(request, "dashboard.html")


@routes.get("/downloads")
async def downloads_page(request: web.Request) -> web.Response:
    # All semantic routes now serve the SPA shell
    return render_template(request, "base_v2.html")


@routes.get("/search")
@routes.get("/explore")
async def search_page(request: web.Request) -> web.Response:
    # Handle both search (legacy) and explore (aurora) paths
    return render_template(request, "base_v2.html")

@routes.get("/discover")
async def discover_page(request: web.Request) -> web.Response:
    # Route to SPA shell
    return render_template(request, "base_v2.html")


@routes.get("/settings")
async def settings_page(request: web.Request) -> web.Response:
    return render_template(request, "base_v2.html")


@routes.get("/design-preview")
async def design_preview(request: web.Request) -> web.Response:
    return render_template(request, "wireframe_search.html")

@routes.get("/design-preview-v2")
async def design_preview_v2(request: web.Request) -> web.Response:
    return render_template(request, "wireframe_search_v2.html")

@routes.get("/v2")
async def frontend_v2(request: web.Request) -> web.Response:
    return render_template(request, "base_v2.html")


@routes.get("/media/{media_type}/{tmdb_id}")
async def media_detail_page(request: web.Request) -> web.Response:
    # Route to SPA shell - Router will handle parsing URL
    return render_template(request, "base_v2.html")


@routes.get("/about")
async def about_page(request: web.Request) -> web.Response:
    import markdown
    readme_content = ""
    # Try to load README.md
    readme_paths = [
        Path("/app/README.md"),
        Path(os.getcwd()) / "README.md",
    ]
    
    for readme_path in readme_paths:
        if readme_path.exists():
            try:
                with open(readme_path, "r", encoding="utf-8") as f:
                    readme_content = f.read()
                break
            except: pass
    
    readme_html = "<p>Documentation not found.</p>"
    if readme_content:
        readme_html = markdown.markdown(readme_content, extensions=["fenced_code", "tables", "toc"])
    
    return render_template(request, "about.html", content=readme_html)

@routes.get("/health")
async def health_check(request: web.Request) -> web.Response:
    return web.json_response({"status": "healthy"})

# Generic Static File Handler is usually done via router.add_static in app setup
