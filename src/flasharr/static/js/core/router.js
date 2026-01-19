/**
 * Flasharr: Core Router
 * 
 * Thin navigation layer that manages view switching and browser history.
 * This replaces the monolithic Router class from app_v2.js.
 */

import { state } from './state.js';
import { ws } from './websocket.js';

/**
 * Minimal router for SPA navigation.
 */
export class Router {
    constructor() {
        this.container = null;
        this.views = new Map();
        this.currentView = null;
        this.currentRoute = null;
    }

    /**
     * Initialize the router.
     * 
     * @param {Object} options - Router options
     * @param {Element} options.container - Main content container
     * @param {Object} options.views - Map of route names to view classes
     */
    init({ container, views = {} }) {
        this.container = container;

        // Register views
        for (const [route, ViewClass] of Object.entries(views)) {
            this.views.set(route, ViewClass);
        }

        // Handle browser back/forward
        window.addEventListener('popstate', (e) => {
            if (e.state && e.state.view) {
                this.navigate(e.state.view, false);
            }
        });

        // Handle link clicks
        document.addEventListener('click', (e) => {
            const link = e.target.closest('[data-route]');
            if (link) {
                e.preventDefault();
                this.navigate(link.dataset.route);
            }
        });

        return this;
    }

    /**
     * Navigate to a route.
     * 
     * @param {string} route - Route name
     * @param {boolean} addToHistory - Whether to add to browser history
     */
    navigate(route, addToHistory = true) {
        if (!this.container) {
            console.error('Router not initialized');
            return;
        }

        // Unmount current view
        if (this.currentView && typeof this.currentView.unmount === 'function') {
            this.currentView.unmount();
        }

        // Get or create view instance
        const ViewClass = this.views.get(route);
        if (!ViewClass) {
            console.warn(`Unknown route: ${route}`);
            return;
        }

        // Create instance if it's a class, otherwise use as-is
        const view = typeof ViewClass === 'function' && ViewClass.prototype
            ? new ViewClass()
            : ViewClass;

        // Mount new view
        if (typeof view.mount === 'function') {
            view.mount(this.container);
        } else if (typeof view.render === 'function') {
            view.render(this.container);
        }

        this.currentView = view;
        this.currentRoute = route;

        // Update browser history
        if (addToHistory) {
            window.history.pushState({ view: route }, '', `/${route}`);
        }

        // Update state
        state.set('currentView', route);

        // Update nav active state
        this.updateNavActiveState(route);
    }

    /**
     * Update navigation active states.
     */
    updateNavActiveState(route) {
        document.querySelectorAll('.nav-item').forEach(item => {
            const itemRoute = item.dataset.route || item.getAttribute('href')?.replace('/', '');
            if (itemRoute === route) {
                item.classList.add('active');
            } else {
                item.classList.remove('active');
            }
        });
    }

    /**
     * Get the container element.
     */
    getContainer() {
        return this.container;
    }

    /**
     * Get current route.
     */
    getCurrentRoute() {
        return this.currentRoute;
    }
}

// Export singleton
export const router = new Router();
