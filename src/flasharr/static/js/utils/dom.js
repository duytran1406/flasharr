/**
 * Flasharr: DOM Utilities
 * 
 * Helper functions for common DOM operations.
 */

import { escapeHtml, escapeAttr } from './sanitize.js';

/**
 * Shorthand for document.getElementById.
 * 
 * @param {string} id - Element ID
 * @returns {Element|null}
 */
export function $(id) {
    return document.getElementById(id);
}

/**
 * Shorthand for document.querySelector.
 * 
 * @param {string} selector - CSS selector
 * @param {Element} context - Context element (default: document)
 * @returns {Element|null}
 */
export function $$(selector, context = document) {
    return context.querySelector(selector);
}

/**
 * Shorthand for document.querySelectorAll with array return.
 * 
 * @param {string} selector - CSS selector
 * @param {Element} context - Context element (default: document)
 * @returns {Element[]}
 */
export function $$$(selector, context = document) {
    return [...context.querySelectorAll(selector)];
}

/**
 * Adds event listener with automatic cleanup.
 * Returns a function to remove the listener.
 * 
 * @param {Element} element - Target element
 * @param {string} event - Event name
 * @param {Function} handler - Event handler
 * @param {Object} options - Event options
 * @returns {Function} - Cleanup function
 */
export function on(element, event, handler, options = {}) {
    element.addEventListener(event, handler, options);
    return () => element.removeEventListener(event, handler, options);
}

/**
 * Adds a one-time event listener.
 * 
 * @param {Element} element - Target element
 * @param {string} event - Event name
 * @param {Function} handler - Event handler
 */
export function once(element, event, handler) {
    element.addEventListener(event, handler, { once: true });
}

/**
 * Delegates event handling to dynamically created children.
 * 
 * @param {Element} parent - Parent element
 * @param {string} event - Event name
 * @param {string} selector - CSS selector for target children
 * @param {Function} handler - Event handler (receives event and matched element)
 * @returns {Function} - Cleanup function
 * 
 * @example
 * delegate(tableBody, 'click', '.action-btn', (e, btn) => {
 *     console.log('Clicked button:', btn.dataset.action);
 * });
 */
export function delegate(parent, event, selector, handler) {
    const delegatedHandler = (e) => {
        const target = e.target.closest(selector);
        if (target && parent.contains(target)) {
            handler(e, target);
        }
    };

    parent.addEventListener(event, delegatedHandler);
    return () => parent.removeEventListener(event, delegatedHandler);
}

/**
 * Shows an element by removing 'hidden' class and setting display.
 * 
 * @param {Element} element - Target element
 * @param {string} display - Display value (default: 'block')
 */
export function show(element, display = 'block') {
    if (element) {
        element.classList.remove('hidden');
        element.style.display = display;
    }
}

/**
 * Hides an element.
 * 
 * @param {Element} element - Target element
 */
export function hide(element) {
    if (element) {
        element.classList.add('hidden');
        element.style.display = 'none';
    }
}

/**
 * Toggles element visibility.
 * 
 * @param {Element} element - Target element
 * @param {boolean} visible - Optional force state
 */
export function toggle(element, visible) {
    if (element) {
        const isVisible = visible !== undefined ? visible : element.classList.contains('hidden');
        isVisible ? show(element) : hide(element);
    }
}

/**
 * Adds a CSS class with optional auto-removal after delay.
 * 
 * @param {Element} element - Target element
 * @param {string} className - Class to add
 * @param {number} duration - Auto-remove after ms (0 = permanent)
 */
export function addClass(element, className, duration = 0) {
    if (element) {
        element.classList.add(className);
        if (duration > 0) {
            setTimeout(() => element.classList.remove(className), duration);
        }
    }
}

/**
 * Removes a CSS class.
 * 
 * @param {Element} element - Target element
 * @param {string} className - Class to remove
 */
export function removeClass(element, className) {
    if (element) {
        element.classList.remove(className);
    }
}

/**
 * Clears all children from an element.
 * 
 * @param {Element} element - Target element
 */
export function empty(element) {
    if (element) {
        while (element.firstChild) {
            element.removeChild(element.firstChild);
        }
    }
}

/**
 * Scrolls an element into view smoothly.
 * 
 * @param {Element} element - Target element
 * @param {Object} options - ScrollIntoView options
 */
export function scrollIntoView(element, options = {}) {
    if (element) {
        element.scrollIntoView({
            behavior: 'smooth',
            block: 'center',
            ...options
        });
    }
}

/**
 * Creates a loading spinner element.
 * 
 * @returns {Element} - Loading spinner element
 */
export function createSpinner() {
    const container = document.createElement('div');
    container.className = 'loading-container';
    container.innerHTML = '<div class="loading-spinner"></div>';
    return container;
}

/**
 * Waits for the next animation frame.
 * 
 * @returns {Promise<number>} - Resolves with timestamp
 */
export function nextFrame() {
    return new Promise(resolve => requestAnimationFrame(resolve));
}

/**
 * Waits for a specified duration.
 * 
 * @param {number} ms - Milliseconds to wait
 * @returns {Promise<void>}
 */
export function wait(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Debounces a function.
 * 
 * @param {Function} fn - Function to debounce
 * @param {number} delay - Delay in ms
 * @returns {Function} - Debounced function
 */
export function debounce(fn, delay) {
    let timeoutId;
    return function (...args) {
        clearTimeout(timeoutId);
        timeoutId = setTimeout(() => fn.apply(this, args), delay);
    };
}

/**
 * Throttles a function.
 * 
 * @param {Function} fn - Function to throttle
 * @param {number} limit - Minimum interval in ms
 * @returns {Function} - Throttled function
 */
export function throttle(fn, limit) {
    let inThrottle;
    return function (...args) {
        if (!inThrottle) {
            fn.apply(this, args);
            inThrottle = true;
            setTimeout(() => inThrottle = false, limit);
        }
    };
}
