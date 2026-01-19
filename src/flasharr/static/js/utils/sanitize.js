/**
 * Flasharr: DOM Sanitization Utilities
 * 
 * Provides secure HTML rendering to prevent XSS attacks.
 * All dynamic content should pass through these functions before
 * being inserted into the DOM.
 */

/**
 * Escapes HTML special characters to prevent XSS.
 * Use this for any user-provided or API-provided text that will be displayed.
 * 
 * @param {string} str - The string to escape
 * @returns {string} - HTML-safe string
 * 
 * @example
 * escapeHtml('<script>alert("xss")</script>') 
 * // Returns: '&lt;script&gt;alert("xss")&lt;/script&gt;'
 */
export function escapeHtml(str) {
    if (str === null || str === undefined) return '';
    if (typeof str !== 'string') str = String(str);

    const escapeMap = {
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#39;',
        '/': '&#x2F;',
        '`': '&#x60;',
        '=': '&#x3D;'
    };

    return str.replace(/[&<>"'`=/]/g, char => escapeMap[char]);
}

/**
 * Escapes a string for use in HTML attributes.
 * More aggressive escaping for attribute contexts.
 * 
 * @param {string} str - The string to escape
 * @returns {string} - Attribute-safe string
 */
export function escapeAttr(str) {
    if (str === null || str === undefined) return '';
    if (typeof str !== 'string') str = String(str);

    // For attributes, we need to escape more aggressively
    return str
        .replace(/&/g, '&amp;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/\\/g, '&#92;')
        .replace(/\n/g, '&#10;')
        .replace(/\r/g, '&#13;');
}

/**
 * Escapes a string for use in JavaScript string literals.
 * Use when injecting values into onclick handlers or similar.
 * 
 * @param {string} str - The string to escape
 * @returns {string} - JS-safe string
 */
export function escapeJs(str) {
    if (str === null || str === undefined) return '';
    if (typeof str !== 'string') str = String(str);

    return str
        .replace(/\\/g, '\\\\')
        .replace(/'/g, "\\'")
        .replace(/"/g, '\\"')
        .replace(/\n/g, '\\n')
        .replace(/\r/g, '\\r')
        .replace(/\t/g, '\\t')
        .replace(/<\/script/gi, '<\\/script');
}

/**
 * Sanitizes a URL to prevent javascript: protocol attacks.
 * 
 * @param {string} url - The URL to sanitize
 * @returns {string} - Safe URL or empty string if dangerous
 */
export function sanitizeUrl(url) {
    if (!url || typeof url !== 'string') return '';

    const trimmed = url.trim().toLowerCase();

    // Block dangerous protocols
    const blockedProtocols = ['javascript:', 'data:', 'vbscript:'];
    if (blockedProtocols.some(proto => trimmed.startsWith(proto))) {
        console.warn('Blocked dangerous URL:', url);
        return '';
    }

    return url;
}

/**
 * Creates a text node safely (no HTML parsing).
 * Use for inserting pure text content.
 * 
 * @param {string} text - The text content
 * @returns {Text} - A text node
 */
export function createTextNode(text) {
    return document.createTextNode(text || '');
}

/**
 * Sets text content of an element safely.
 * This is the safest way to set text - no HTML parsing occurs.
 * 
 * @param {Element} element - The target element
 * @param {string} text - The text to set
 */
export function setTextContent(element, text) {
    if (element) {
        element.textContent = text || '';
    }
}

/**
 * Creates an element with attributes and text content safely.
 * 
 * @param {string} tag - Tag name
 * @param {Object} attrs - Attributes to set
 * @param {string} textContent - Optional text content
 * @returns {Element} - The created element
 * 
 * @example
 * createElement('a', { href: '/downloads', class: 'nav-link' }, 'Downloads')
 */
export function createElement(tag, attrs = {}, textContent = null) {
    const el = document.createElement(tag);

    for (const [key, value] of Object.entries(attrs)) {
        if (key === 'class') {
            el.className = value;
        } else if (key === 'style' && typeof value === 'object') {
            Object.assign(el.style, value);
        } else if (key.startsWith('data-')) {
            el.dataset[key.slice(5)] = value;
        } else if (key === 'href' || key === 'src') {
            el.setAttribute(key, sanitizeUrl(value));
        } else {
            el.setAttribute(key, value);
        }
    }

    if (textContent !== null) {
        el.textContent = textContent;
    }

    return el;
}

/**
 * Allowed tags and attributes for HTML sanitization.
 * Extend this list as needed for your application.
 */
const ALLOWED_TAGS = new Set([
    'div', 'span', 'p', 'a', 'b', 'i', 'strong', 'em', 'br', 'hr',
    'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
    'ul', 'ol', 'li',
    'table', 'thead', 'tbody', 'tr', 'th', 'td',
    'img', 'button', 'input', 'label', 'select', 'option',
    'form', 'textarea'
]);

const ALLOWED_ATTRS = new Set([
    'class', 'id', 'style', 'title', 'alt', 'src', 'href',
    'type', 'value', 'placeholder', 'disabled', 'readonly',
    'data-*', 'aria-*', 'role'
]);

/**
 * Sanitizes HTML by removing dangerous tags and attributes.
 * Use sparingly - prefer escapeHtml + textContent for most cases.
 * 
 * @param {string} html - The HTML to sanitize
 * @returns {string} - Sanitized HTML
 */
export function sanitizeHtml(html) {
    if (!html || typeof html !== 'string') return '';

    // Create a temporary container
    const temp = document.createElement('div');
    temp.innerHTML = html;

    // Walk the DOM and remove dangerous elements
    const walk = (node) => {
        const children = [...node.childNodes];

        for (const child of children) {
            if (child.nodeType === Node.ELEMENT_NODE) {
                const tag = child.tagName.toLowerCase();

                // Remove script, style, and other dangerous tags
                if (!ALLOWED_TAGS.has(tag) || tag === 'script' || tag === 'style') {
                    child.remove();
                    continue;
                }

                // Remove dangerous attributes
                const attrs = [...child.attributes];
                for (const attr of attrs) {
                    const name = attr.name.toLowerCase();

                    // Remove event handlers
                    if (name.startsWith('on')) {
                        child.removeAttribute(attr.name);
                        continue;
                    }

                    // Check javascript: URLs
                    if ((name === 'href' || name === 'src') &&
                        attr.value.toLowerCase().trim().startsWith('javascript:')) {
                        child.removeAttribute(attr.name);
                    }
                }

                walk(child);
            }
        }
    };

    walk(temp);
    return temp.innerHTML;
}

/**
 * Tagged template literal for safe HTML.
 * Automatically escapes interpolated values.
 * 
 * @example
 * const name = '<script>alert("xss")</script>';
 * element.innerHTML = html`<div>Hello, ${name}!</div>`;
 * // Result: <div>Hello, &lt;script&gt;alert("xss")&lt;/script&gt;!</div>
 */
export function html(strings, ...values) {
    return strings.reduce((result, str, i) => {
        const value = values[i - 1];
        const escaped = value !== undefined ? escapeHtml(value) : '';
        return result + escaped + str;
    });
}

/**
 * Tagged template literal that allows raw HTML for specific marked values.
 * Use htmlRaw() to mark values that should not be escaped.
 * 
 * @example
 * const icon = htmlRaw('<span class="material-icons">check</span>');
 * element.innerHTML = safeHtml`<div>${icon} ${userInput}</div>`;
 */
const RAW_MARKER = Symbol('raw-html');

export function htmlRaw(html) {
    return { [RAW_MARKER]: html };
}

export function safeHtml(strings, ...values) {
    return strings.reduce((result, str, i) => {
        const value = values[i - 1];
        let escaped;

        if (value === undefined) {
            escaped = '';
        } else if (value && value[RAW_MARKER]) {
            escaped = value[RAW_MARKER]; // Trust this HTML
        } else {
            escaped = escapeHtml(value);
        }

        return result + escaped + str;
    });
}
