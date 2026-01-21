/**
 * ERROR HANDLING AND HEALTH INDICATOR FUNCTIONS
 * Add these functions to settings.html <script> section
 */

// Show error banner
function showErrorBanner(title, message) {
    const banner = document.getElementById('account-error-banner');
    const titleEl = document.getElementById('account-error-title');
    const messageEl = document.getElementById('account-error-message');

    if (banner && titleEl && messageEl) {
        titleEl.textContent = title || 'Error';
        messageEl.textContent = message || 'An error occurred';
        banner.style.display = 'flex';

        // Auto-hide after 10 seconds
        setTimeout(() => {
            banner.style.display = 'none';
        }, 10000);
    }
}

// Close error banner
function closeErrorBanner() {
    const banner = document.getElementById('account-error-banner');
    if (banner) {
        banner.style.display = 'none';
    }
}

// Update health indicators
function updateHealthIndicators(accounts, primary) {
    const authHealth = document.getElementById('auth-health');
    const quotaHealth = document.getElementById('quota-health');
    const sessionHealth = document.getElementById('session-health');

    if (!authHealth || !quotaHealth || !sessionHealth) return;

    // Reset classes
    [authHealth, quotaHealth, sessionHealth].forEach(el => {
        el.classList.remove('health-ok', 'health-warning', 'health-error');
    });

    // Authentication Health
    if (accounts && accounts.length > 0) {
        const hasValidAccount = accounts.some(acc => acc.premium || acc.traffic_left);
        if (hasValidAccount) {
            authHealth.classList.add('health-ok');
            authHealth.querySelector('.health-status').textContent = `${accounts.length} Account(s)`;
        } else {
            authHealth.classList.add('health-warning');
            authHealth.querySelector('.health-status').textContent = 'No Valid Accounts';
        }
    } else {
        authHealth.classList.add('health-error');
        authHealth.querySelector('.health-status').textContent = 'Not Configured';
    }

    // Quota Health
    if (primary && primary.traffic_left) {
        const trafficParts = primary.traffic_left.split('/');
        if (trafficParts.length === 2) {
            const used = parseFloat(trafficParts[0]);
            const total = parseFloat(trafficParts[1]);
            const remaining = total - used;
            const percent = (remaining / total) * 100;

            if (percent > 20) {
                quotaHealth.classList.add('health-ok');
            } else if (percent > 5) {
                quotaHealth.classList.add('health-warning');
            } else {
                quotaHealth.classList.add('health-error');
            }

            quotaHealth.querySelector('.health-status').textContent = `${remaining.toFixed(1)} GB Left`;
        } else {
            quotaHealth.querySelector('.health-status').textContent = primary.traffic_left;
            quotaHealth.classList.add('health-ok');
        }
    } else {
        quotaHealth.querySelector('.health-status').textContent = 'N/A';
    }

    // Session Health
    if (primary) {
        sessionHealth.classList.add('health-ok');
        sessionHealth.querySelector('.health-status').textContent = 'Active';
    } else {
        sessionHealth.classList.add('health-warning');
        sessionHealth.querySelector('.health-status').textContent = 'No Primary';
    }
}

// Enhanced account rendering with error display
function renderAccountsWithErrors(accounts, primary) {
    // Call original renderAccounts
    if (window.renderAccounts) {
        window.renderAccounts(accounts, primary);
    }

    // Update health indicators
    updateHealthIndicators(accounts, primary);

    // Check for account errors
    if (accounts && accounts.length > 0) {
        const errorAccounts = accounts.filter(acc => acc.error || acc.last_error);
        if (errorAccounts.length > 0) {
            const errorMessages = errorAccounts.map(acc =>
                `${acc.email}: ${acc.error || acc.last_error}`
            ).join('; ');
            showErrorBanner('Account Errors', errorMessages);
        }
    }
}

// Enhanced login modal with error handling
async function handleModalLogin() {
    const email = document.getElementById('modal_email').value;
    const password = document.getElementById('modal_password').value;
    const status = document.getElementById('modal-status');
    const btn = document.getElementById('btn-modal-login');

    if (!email || !password) {
        status.textContent = 'Email and password required';
        status.className = 'connection-status error';
        status.style.display = 'flex';
        return;
    }

    btn.disabled = true;
    btn.innerHTML = '<span class="material-icons spin">sync</span> Logging in...';
    status.innerHTML = '<span class="material-icons spin">sync</span> authenticating...';
    status.className = 'connection-status testing';
    status.style.display = 'flex';

    try {
        const response = await fetch('/api/accounts/add', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email, password })
        });
        const data = await response.json();

        if (data.status === 'ok') {
            showToast('Account added successfully');
            hideLoginModal();
            loadAccounts();
        } else {
            // Show error in modal AND banner
            const errorMsg = data.message || 'Login failed';
            status.textContent = errorMsg;
            status.className = 'connection-status error';
            showErrorBanner('Login Failed', errorMsg);
        }
    } catch (error) {
        const errorMsg = 'Connection error: ' + error.message;
        status.textContent = errorMsg;
        status.className = 'connection-status error';
        showErrorBanner('Connection Error', errorMsg);
    } finally {
        btn.disabled = false;
        btn.innerHTML = '<span class="material-icons">login</span> Login';
    }
}

// Enhanced loadAccounts with error handling
async function loadAccountsWithErrorHandling() {
    console.log('Fetching accounts...');
    try {
        const response = await fetch('/api/accounts');
        if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);

        const data = await response.json();
        console.log('Accounts loaded:', data);

        if (data.status === 'ok') {
            renderAccountsWithErrors(data.accounts, data.primary);
        } else {
            throw new Error(data.message || 'Unknown server error');
        }
    } catch (error) {
        console.error('Error loading accounts:', error);
        showErrorBanner('Failed to Load Accounts', error.message);

        const accountsContainer = document.getElementById('accounts-container');
        if (accountsContainer) {
            accountsContainer.innerHTML = `
                <div class="empty-accounts" style="border-color: rgba(239, 68, 68, 0.2);">
                    <span class="material-icons" style="color: #ef4444;">error</span>
                    <p>Failed to load accounts: ${error.message}</p>
                    <button class="btn btn-secondary" onclick="loadAccountsWithErrorHandling()">
                        <span class="material-icons">refresh</span>
                        Retry
                    </button>
                </div>
            `;
        }
    }
}

// Export functions to window
window.showErrorBanner = showErrorBanner;
window.closeErrorBanner = closeErrorBanner;
window.updateHealthIndicators = updateHealthIndicators;
window.renderAccountsWithErrors = renderAccountsWithErrors;
window.loadAccountsWithErrorHandling = loadAccountsWithErrorHandling;
window.handleModalLogin = handleModalLogin;
