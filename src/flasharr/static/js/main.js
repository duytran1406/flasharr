/**
 * Flasharr: Main Entry Point
 * 
 * Initializes the modular application using ES modules.
 * This file bootstraps the router and connects all components.
 */

import { router, ws, state } from './core/index.js';
import {
    DownloadsView,
    DashboardView,
    SettingsView,
    modal
} from './components/index.js';

/**
 * Initialize the application.
 */
function init() {
    // Get the main container
    const container = document.getElementById('main-content') ||
        document.querySelector('.view-container');

    if (!container) {
        console.error('Main container not found');
        return;
    }

    // Initialize router with views
    router.init({
        container,
        views: {
            'dashboard': DashboardView,
            'downloads': DownloadsView,
            'settings': SettingsView,
            // 'discover': DiscoverView,  // TODO
        }
    });

    // Connect WebSocket
    ws.connect();

    // Initialize modal system
    modal.init();

    // Navigate to initial route
    const path = window.location.pathname.replace(/^\//, '') || 'dashboard';
    const validRoutes = ['dashboard', 'downloads', 'settings', 'discover', 'explore'];
    const initialRoute = validRoutes.includes(path) ? path : 'dashboard';

    router.navigate(initialRoute, false);

    // Expose router globally for legacy compatibility
    window.router = router;

    console.log('✅ Flasharr modular app initialized');
}

// Initialize when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
