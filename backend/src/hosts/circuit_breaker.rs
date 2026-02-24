// Circuit Breaker Implementation for Fshare API
// Prevents unlimited retries when Fshare API is down

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,    // Normal operation
    Open,      // Blocking requests (service down)
    HalfOpen,  // Testing recovery
}

pub struct CircuitBreaker {
    state: Mutex<CircuitState>,
    failure_count: AtomicUsize,
    last_failure_time: Mutex<Option<Instant>>,
    success_count: AtomicUsize, // For half-open state
}

impl CircuitBreaker {
    const FAILURE_THRESHOLD: usize = 5;        // Open after 5 failures
    const TIMEOUT: Duration = Duration::from_secs(60); // Wait 60s before half-open
    const HALF_OPEN_SUCCESS_THRESHOLD: usize = 2; // Close after 2 successes in half-open
    
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CircuitState::Closed),
            failure_count: AtomicUsize::new(0),
            last_failure_time: Mutex::new(None),
            success_count: AtomicUsize::new(0),
        }
    }
    
    /// Check if request is allowed
    pub async fn is_request_allowed(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;
        
        match *state {
            CircuitState::Closed => {
                // Normal operation, allow request
                Ok(())
            }
            CircuitState::Open => {
                // Check if timeout has elapsed
                let last_failure = self.last_failure_time.lock().await;
                if let Some(last) = *last_failure {
                    if last.elapsed() >= Self::TIMEOUT {
                        // Timeout elapsed, transition to half-open
                        *state = CircuitState::HalfOpen;
                        self.success_count.store(0, Ordering::SeqCst);
                        tracing::info!("[CIRCUIT_BREAKER] Transitioning to HALF_OPEN (testing recovery)");
                        drop(state);
                        drop(last_failure);
                        Ok(())
                    } else {
                        // Still in timeout period
                        let remaining = Self::TIMEOUT - last.elapsed();
                        Err(format!(
                            "Circuit breaker OPEN (Fshare API unavailable). Retry in {}s",
                            remaining.as_secs()
                        ))
                    }
                } else {
                    // Should not happen, but allow request
                    Ok(())
                }
            }
            CircuitState::HalfOpen => {
                // Allow limited requests to test recovery
                Ok(())
            }
        }
    }
    
    /// Record successful request
    pub async fn record_success(&self) {
        let mut state = self.state.lock().await;
        
        match *state {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                // Increment success count
                let successes = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                
                if successes >= Self::HALF_OPEN_SUCCESS_THRESHOLD {
                    // Enough successes, close circuit
                    *state = CircuitState::Closed;
                    self.failure_count.store(0, Ordering::SeqCst);
                    self.success_count.store(0, Ordering::SeqCst);
                    tracing::info!("[CIRCUIT_BREAKER] Transitioning to CLOSED (service recovered)");
                } else {
                    tracing::info!(
                        "[CIRCUIT_BREAKER] Success in HALF_OPEN ({}/{})",
                        successes,
                        Self::HALF_OPEN_SUCCESS_THRESHOLD
                    );
                }
            }
            CircuitState::Open => {
                // Should not happen (requests blocked in open state)
                tracing::warn!("[CIRCUIT_BREAKER] Unexpected success in OPEN state");
            }
        }
    }
    
    /// Record failed request
    pub async fn record_failure(&self) {
        let mut state = self.state.lock().await;
        
        match *state {
            CircuitState::Closed => {
                // Increment failure count
                let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                
                if failures >= Self::FAILURE_THRESHOLD {
                    // Too many failures, open circuit
                    *state = CircuitState::Open;
                    let mut last_failure = self.last_failure_time.lock().await;
                    *last_failure = Some(Instant::now());
                    tracing::error!(
                        "[CIRCUIT_BREAKER] Transitioning to OPEN ({} consecutive failures)",
                        failures
                    );
                } else {
                    tracing::warn!(
                        "[CIRCUIT_BREAKER] Failure in CLOSED ({}/{})",
                        failures,
                        Self::FAILURE_THRESHOLD
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Failure in half-open, go back to open
                *state = CircuitState::Open;
                let mut last_failure = self.last_failure_time.lock().await;
                *last_failure = Some(Instant::now());
                self.success_count.store(0, Ordering::SeqCst);
                tracing::error!("[CIRCUIT_BREAKER] Transitioning back to OPEN (recovery failed)");
            }
            CircuitState::Open => {
                // Already open, just update timestamp
                let mut last_failure = self.last_failure_time.lock().await;
                *last_failure = Some(Instant::now());
            }
        }
    }
}

// ============================================================================
// WHY THIS PREVENTS BANS
// ============================================================================

/*
PROBLEM: Unlimited retries when Fshare API is down

SCENARIO WITHOUT CIRCUIT BREAKER:
1. Fshare API goes down (maintenance, network issue, etc.)
2. Download 1 fails → Retry
3. Download 2 fails → Retry
4. Download 3 fails → Retry
5. ... 100 downloads fail → 100 retries
6. Fshare sees 100+ rapid requests from same IP
7. Account banned for bot-like behavior

SCENARIO WITH CIRCUIT BREAKER:
1. Fshare API goes down
2. Download 1 fails → Failure count: 1/5
3. Download 2 fails → Failure count: 2/5
4. Download 3 fails → Failure count: 3/5
5. Download 4 fails → Failure count: 4/5
6. Download 5 fails → Failure count: 5/5 → CIRCUIT OPENS
7. Downloads 6-100 → Blocked immediately with clear error message
8. After 60 seconds → Circuit transitions to HALF_OPEN
9. Download 101 → Test request (allowed)
10. If success → Circuit CLOSES (service recovered)
11. If failure → Circuit stays OPEN (wait another 60s)

BENEFITS:
✅ Prevents rapid retry loops (max 5 failures before blocking)
✅ Protects against account bans (no spam when API is down)
✅ Clear error messages ("Circuit breaker OPEN, retry in 45s")
✅ Automatic recovery testing (half-open state)
✅ Graceful degradation (fail fast instead of hanging)

CONFIGURATION:
- FAILURE_THRESHOLD: 5 failures (conservative)
- TIMEOUT: 60 seconds (reasonable for API recovery)
- HALF_OPEN_SUCCESS_THRESHOLD: 2 successes (confirm recovery)

STATE TRANSITIONS:
CLOSED --[5 failures]--> OPEN
OPEN --[60s timeout]--> HALF_OPEN
HALF_OPEN --[2 successes]--> CLOSED
HALF_OPEN --[1 failure]--> OPEN
*/
