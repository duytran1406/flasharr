/**
 * Frontend Unit Tests - Node.js Compatible
 * 
 * Tests for utility and core modules that don't require DOM.
 * Run with: node --experimental-vm-modules tests/frontend/test_utils.mjs
 */

import { strict as assert } from 'assert';

// Mock window for modules that check for it
global.window = { location: { origin: 'http://localhost' } };

// Dynamic imports to handle ES modules
const sanitize = await import('../../src/flasharr/static/js/utils/sanitize.js');
const format = await import('../../src/flasharr/static/js/utils/format.js');

let passed = 0;
let failed = 0;

function test(name, fn) {
    try {
        fn();
        console.log(`  ✅ ${name}`);
        passed++;
    } catch (e) {
        console.log(`  ❌ ${name}`);
        console.log(`     Error: ${e.message}`);
        failed++;
    }
}

function suite(name, fn) {
    console.log(`\n📦 ${name}`);
    fn();
}

// =============================================================================
// SANITIZE.JS TESTS
// =============================================================================

suite('sanitize.js - escapeHtml', () => {
    test('escapes < and > characters', () => {
        const result = sanitize.escapeHtml('<script>alert("xss")</script>');
        assert(result.includes('&lt;'), 'Should escape <');
        assert(result.includes('&gt;'), 'Should escape >');
    });

    test('escapes double quotes', () => {
        const result = sanitize.escapeHtml('Hello "world"');
        assert(result.includes('&quot;'), 'Should escape "');
    });

    test('escapes single quotes', () => {
        const result = sanitize.escapeHtml("Hello 'world'");
        assert(result.includes('&#39;'), "Should escape '");
    });

    test('escapes ampersands', () => {
        const result = sanitize.escapeHtml('Tom & Jerry');
        assert(result.includes('&amp;'), 'Should escape &');
    });

    test('handles null', () => {
        assert.equal(sanitize.escapeHtml(null), '');
    });

    test('handles undefined', () => {
        assert.equal(sanitize.escapeHtml(undefined), '');
    });

    test('converts numbers to strings', () => {
        assert.equal(sanitize.escapeHtml(123), '123');
    });

    test('preserves safe text', () => {
        assert.equal(sanitize.escapeHtml('Hello World'), 'Hello World');
    });
});

suite('sanitize.js - escapeAttr', () => {
    test('escapes attribute-specific characters', () => {
        const result = sanitize.escapeAttr('onclick="alert(1)"');
        assert(result.includes('&quot;'), 'Should escape quotes');
    });

    test('escapes newlines', () => {
        const result = sanitize.escapeAttr('line1\nline2');
        assert(result.includes('&#10;'), 'Should escape newline');
    });

    test('escapes carriage returns', () => {
        const result = sanitize.escapeAttr('line1\rline2');
        assert(result.includes('&#13;'), 'Should escape carriage return');
    });
});

suite('sanitize.js - escapeJs', () => {
    test('escapes single quotes', () => {
        const result = sanitize.escapeJs("it's");
        assert(result.includes("\\'"), "Should escape '");
    });

    test('escapes double quotes', () => {
        const result = sanitize.escapeJs('say "hello"');
        assert(result.includes('\\"'), 'Should escape "');
    });

    test('escapes backslashes', () => {
        const result = sanitize.escapeJs('path\\to\\file');
        assert(result.includes('\\\\'), 'Should escape \\');
    });

    test('escapes script closing tags', () => {
        const result = sanitize.escapeJs('</script>');
        assert(!result.includes('</script>'), 'Should break script tag');
    });
});

suite('sanitize.js - sanitizeUrl', () => {
    test('blocks javascript: protocol', () => {
        assert.equal(sanitize.sanitizeUrl('javascript:alert(1)'), '');
    });

    test('blocks JavaScript: (case insensitive)', () => {
        assert.equal(sanitize.sanitizeUrl('JavaScript:alert(1)'), '');
    });

    test('blocks data: protocol', () => {
        assert.equal(sanitize.sanitizeUrl('data:text/html,<script>'), '');
    });

    test('blocks vbscript: protocol', () => {
        assert.equal(sanitize.sanitizeUrl('vbscript:msgbox'), '');
    });

    test('allows https URLs', () => {
        const url = 'https://example.com/path?q=1';
        assert.equal(sanitize.sanitizeUrl(url), url);
    });

    test('allows http URLs', () => {
        const url = 'http://example.com';
        assert.equal(sanitize.sanitizeUrl(url), url);
    });

    test('allows relative URLs', () => {
        assert.equal(sanitize.sanitizeUrl('/api/downloads'), '/api/downloads');
    });

    test('handles null/undefined', () => {
        assert.equal(sanitize.sanitizeUrl(null), '');
        assert.equal(sanitize.sanitizeUrl(undefined), '');
    });
});

// =============================================================================
// FORMAT.JS TESTS
// =============================================================================

suite('format.js - formatBytes', () => {
    test('formats 0 bytes', () => {
        assert.equal(format.formatBytes(0), '0 B');
    });

    test('formats bytes', () => {
        assert.equal(format.formatBytes(512), '512 B');
    });

    test('formats kilobytes', () => {
        assert.equal(format.formatBytes(1024), '1 KB');
    });

    test('formats megabytes', () => {
        assert.equal(format.formatBytes(1048576), '1 MB');
    });

    test('formats gigabytes', () => {
        assert.equal(format.formatBytes(1073741824), '1 GB');
    });

    test('formats terabytes', () => {
        assert.equal(format.formatBytes(1099511627776), '1 TB');
    });

    test('handles decimal precision', () => {
        assert.equal(format.formatBytes(1536, 1), '1.5 KB');
    });

    test('handles null', () => {
        assert.equal(format.formatBytes(null), '0 B');
    });

    test('handles undefined', () => {
        assert.equal(format.formatBytes(undefined), '0 B');
    });

    test('handles NaN', () => {
        assert.equal(format.formatBytes(NaN), '0 B');
    });
});

suite('format.js - formatSpeed', () => {
    test('formats speed in MB/s', () => {
        assert.equal(format.formatSpeed(1048576), '1 MB/s');
    });

    test('handles zero', () => {
        assert.equal(format.formatSpeed(0), '0 B/s');
    });

    test('handles negative', () => {
        assert.equal(format.formatSpeed(-100), '0 B/s');
    });
});

suite('format.js - formatDuration', () => {
    test('formats seconds', () => {
        assert.equal(format.formatDuration(45), '45s');
    });

    test('formats minutes and seconds', () => {
        assert.equal(format.formatDuration(125), '2m 5s');
    });

    test('formats hours and minutes', () => {
        assert.equal(format.formatDuration(3725), '1h 2m');
    });

    test('formats days for very long durations', () => {
        const result = format.formatDuration(90000); // 25 hours
        assert(result.includes('d'), 'Should include days');
    });

    test('handles zero', () => {
        assert.equal(format.formatDuration(0), '--');
    });

    test('handles negative', () => {
        assert.equal(format.formatDuration(-10), '--');
    });

    test('handles Infinity', () => {
        assert.equal(format.formatDuration(Infinity), '--');
    });
});

suite('format.js - formatPercent', () => {
    test('formats whole percentages', () => {
        assert.equal(format.formatPercent(50), '50.0%');
    });

    test('formats decimal percentages', () => {
        assert.equal(format.formatPercent(99.99, 2), '99.99%');
    });

    test('handles null', () => {
        assert.equal(format.formatPercent(null), '0%');
    });
});

suite('format.js - truncate', () => {
    test('truncates long strings', () => {
        assert.equal(format.truncate('Hello World', 8), 'Hello...');
    });

    test('returns short strings unchanged', () => {
        assert.equal(format.truncate('Hi', 10), 'Hi');
    });

    test('handles empty strings', () => {
        assert.equal(format.truncate('', 10), '');
    });

    test('handles null', () => {
        assert.equal(format.truncate(null, 10), '');
    });

    test('supports custom suffix', () => {
        assert.equal(format.truncate('Hello World', 9, '…'), 'Hello Wo…');
    });
});

suite('format.js - capitalize', () => {
    test('capitalizes first letter', () => {
        assert.equal(format.capitalize('hello'), 'Hello');
    });

    test('handles empty string', () => {
        assert.equal(format.capitalize(''), '');
    });

    test('handles null', () => {
        assert.equal(format.capitalize(null), '');
    });

    test('preserves rest of string', () => {
        assert.equal(format.capitalize('hELLO'), 'HELLO');
    });
});

suite('format.js - formatNumber', () => {
    test('formats large numbers with separators', () => {
        const result = format.formatNumber(1000000);
        // Result depends on locale, but should contain separators
        assert(result.length > 6, 'Should have separators');
    });

    test('handles null', () => {
        assert.equal(format.formatNumber(null), '0');
    });
});

// =============================================================================
// SUMMARY
// =============================================================================

console.log('\n' + '='.repeat(50));
console.log(`📊 Results: ${passed} passed, ${failed} failed`);
console.log('='.repeat(50));

if (failed > 0) {
    process.exit(1);
}
