//! Intelligent Error Classification
//!
//! Classifies download errors into categories and determines appropriate recovery actions.
//! No blind retries - each error type gets smart handling.

use serde::{Deserialize, Serialize};

/// Error category with recovery strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorCategory {
    /// Temporary network/server issue - retry with same URL
    Retryable {
        max_retries: u32,
        delay_seconds: u64,
        reason: String,
    },
    
    /// Premium URL expired - need to refresh from original URL
    UrlRefreshNeeded {
        max_retries: u32,
        reason: String,
    },
    
    /// Account issue - needs user intervention
    AccountIssue {
        reason: String,
        action_required: String,
    },
    
    /// Permanent failure - will never succeed
    Permanent {
        reason: String,
    },
    
    /// System/local issue - check configuration
    SystemIssue {
        max_retries: u32,
        reason: String,
        fix_suggestion: String,
    },
}

/// Error classifier - analyzes errors and determines recovery strategy
pub struct ErrorClassifier;

impl ErrorClassifier {
    /// Classify an error and determine recovery strategy
    pub fn classify(error: &anyhow::Error) -> ErrorCategory {
        let error_str = error.to_string().to_lowercase();
        
        // Try HTTP status code first
        if let Some(status) = Self::extract_http_status(&error_str) {
            return Self::classify_http_error(status, &error_str);
        }
        
        // Network errors
        if error_str.contains("timeout") || error_str.contains("timed out") {
            return ErrorCategory::Retryable {
                max_retries: 10,
                delay_seconds: 5,
                reason: "Network timeout - connection too slow".to_string(),
            };
        }
        
        if error_str.contains("connection reset") || error_str.contains("broken pipe") {
            return ErrorCategory::Retryable {
                max_retries: 10,
                delay_seconds: 3,
                reason: "Connection reset by server".to_string(),
            };
        }
        
        if error_str.contains("connection refused") {
            return ErrorCategory::Retryable {
                max_retries: 5,
                delay_seconds: 10,
                reason: "Server refused connection - may be down".to_string(),
            };
        }
        
        // DNS errors
        if error_str.contains("dns") || error_str.contains("resolve") || error_str.contains("name resolution") {
            return ErrorCategory::SystemIssue {
                max_retries: 5,
                reason: "DNS resolution failed".to_string(),
                fix_suggestion: "Check your internet connection and DNS settings".to_string(),
            };
        }
        
        // Network unreachable
        if error_str.contains("no route") || error_str.contains("network unreachable") || error_str.contains("network is unreachable") {
            return ErrorCategory::SystemIssue {
                max_retries: 10,
                reason: "No internet connection".to_string(),
                fix_suggestion: "Check your network connection".to_string(),
            };
        }
        
        // Disk errors
        if error_str.contains("no space") || error_str.contains("disk full") {
            return ErrorCategory::Permanent {
                reason: "Disk full - no space left on device".to_string(),
            };
        }
        
        if error_str.contains("permission denied") {
            return ErrorCategory::Permanent {
                reason: "Permission denied - cannot write to destination".to_string(),
            };
        }
        
        // SSL/TLS errors
        if error_str.contains("ssl") || error_str.contains("tls") || error_str.contains("certificate") {
            return ErrorCategory::SystemIssue {
                max_retries: 3,
                reason: "SSL/TLS error".to_string(),
                fix_suggestion: "Check system time and SSL certificates".to_string(),
            };
        }
        
        // Default: treat as retryable with conservative settings
        ErrorCategory::Retryable {
            max_retries: 3,
            delay_seconds: 5,
            reason: format!("Unknown error: {}", error),
        }
    }
    
    /// Classify HTTP status code errors
    fn classify_http_error(status: u16, error_str: &str) -> ErrorCategory {
        match status {
            // 2xx Success (shouldn't be errors, but handle anyway)
            200..=299 => ErrorCategory::Retryable {
                max_retries: 1,
                delay_seconds: 1,
                reason: format!("Unexpected success code {}", status),
            },
            
            // 3xx Redirection
            300..=399 => ErrorCategory::Retryable {
                max_retries: 3,
                delay_seconds: 2,
                reason: format!("Redirect error {}", status),
            },
            
            // 400 Bad Request
            400 => ErrorCategory::Permanent {
                reason: "Bad request - invalid URL or parameters".to_string(),
            },
            
            // 401 Unauthorized - might be expired token
            401 => {
                if error_str.contains("token") || error_str.contains("session") {
                    ErrorCategory::UrlRefreshNeeded {
                        max_retries: 3,
                        reason: "Authentication token expired".to_string(),
                    }
                } else {
                    ErrorCategory::AccountIssue {
                        reason: "Authentication failed".to_string(),
                        action_required: "Check your account credentials".to_string(),
                    }
                }
            },
            
            // 402 Payment Required
            402 => ErrorCategory::AccountIssue {
                reason: "Insufficient credits or payment required".to_string(),
                action_required: "Add credits to your account".to_string(),
            },
            
            // 403 Forbidden
            403 => {
                if error_str.contains("expired") || error_str.contains("token") {
                    ErrorCategory::UrlRefreshNeeded {
                        max_retries: 3,
                        reason: "Premium link expired (6h limit exceeded)".to_string(),
                    }
                } else if error_str.contains("suspended") || error_str.contains("banned") {
                    ErrorCategory::AccountIssue {
                        reason: "Account suspended or banned".to_string(),
                        action_required: "Contact support".to_string(),
                    }
                } else {
                    ErrorCategory::Permanent {
                        reason: "Access forbidden".to_string(),
                    }
                }
            },
            
            // 404 Not Found
            404 => {
                if error_str.contains("file") {
                    ErrorCategory::Permanent {
                        reason: "File deleted from server".to_string(),
                    }
                } else {
                    ErrorCategory::UrlRefreshNeeded {
                        max_retries: 3,
                        reason: "Premium URL no longer valid".to_string(),
                    }
                }
            },
            
            // 408 Request Timeout
            408 => ErrorCategory::Retryable {
                max_retries: 10,
                delay_seconds: 5,
                reason: "Request timeout".to_string(),
            },
            
            // 410 Gone
            410 => ErrorCategory::UrlRefreshNeeded {
                max_retries: 3,
                reason: "URL expired or no longer available".to_string(),
            },
            
            // 429 Too Many Requests
            429 => ErrorCategory::Retryable {
                max_retries: 10,
                delay_seconds: 30,
                reason: "Rate limited - too many requests".to_string(),
            },
            
            // 451 Unavailable For Legal Reasons
            451 => ErrorCategory::Permanent {
                reason: "File removed due to copyright claim (DMCA)".to_string(),
            },
            
            // 500 Internal Server Error
            500 => ErrorCategory::Retryable {
                max_retries: 5,
                delay_seconds: 10,
                reason: "Server internal error".to_string(),
            },
            
            // 502 Bad Gateway
            502 => ErrorCategory::Retryable {
                max_retries: 10,
                delay_seconds: 5,
                reason: "Bad gateway - upstream server issue".to_string(),
            },
            
            // 503 Service Unavailable
            503 => ErrorCategory::Retryable {
                max_retries: 10,
                delay_seconds: 10,
                reason: "Server temporarily unavailable or overloaded".to_string(),
            },
            
            // 504 Gateway Timeout
            504 => ErrorCategory::Retryable {
                max_retries: 10,
                delay_seconds: 15,
                reason: "Gateway timeout - upstream server too slow".to_string(),
            },
            
            // Other 4xx errors (client errors)
            400..=499 => ErrorCategory::Permanent {
                reason: format!("Client error: HTTP {}", status),
            },
            
            // Other 5xx errors (server errors)
            500..=599 => ErrorCategory::Retryable {
                max_retries: 5,
                delay_seconds: 10,
                reason: format!("Server error: HTTP {}", status),
            },
            
            // Unknown status codes
            _ => ErrorCategory::Retryable {
                max_retries: 3,
                delay_seconds: 5,
                reason: format!("Unknown HTTP status: {}", status),
            },
        }
    }
    
    /// Extract HTTP status code from error string
    fn extract_http_status(error_str: &str) -> Option<u16> {
        // Try different patterns
        let patterns = [
            "http error: ",
            "http ",
            "status code ",
            "status: ",
            "code ",
        ];
        
        for pattern in &patterns {
            if let Some(start) = error_str.find(pattern) {
                let after_pattern = &error_str[start + pattern.len()..];
                
                // Take first word/number
                let status_str: String = after_pattern
                    .chars()
                    .take_while(|c| c.is_numeric())
                    .collect();
                
                if let Ok(status) = status_str.parse::<u16>() {
                    return Some(status);
                }
            }
        }
        
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_classify_timeout() {
        let error = anyhow::anyhow!("connection timeout");
        let category = ErrorClassifier::classify(&error);
        
        match category {
            ErrorCategory::Retryable { max_retries, .. } => {
                assert_eq!(max_retries, 10);
            }
            _ => panic!("Expected Retryable"),
        }
    }
    
    #[test]
    fn test_classify_404() {
        let error = anyhow::anyhow!("HTTP error: 404 file not found");
        let category = ErrorClassifier::classify(&error);
        
        match category {
            ErrorCategory::Permanent { reason } => {
                assert!(reason.contains("deleted"));
            }
            _ => panic!("Expected Permanent"),
        }
    }
    
    #[test]
    fn test_classify_403_expired() {
        let error = anyhow::anyhow!("HTTP 403: token expired");
        let category = ErrorClassifier::classify(&error);
        
        match category {
            ErrorCategory::UrlRefreshNeeded { max_retries, .. } => {
                assert_eq!(max_retries, 3);
            }
            _ => panic!("Expected UrlRefreshNeeded"),
        }
    }
    
    #[test]
    fn test_classify_429_rate_limit() {
        let error = anyhow::anyhow!("HTTP status code 429");
        let category = ErrorClassifier::classify(&error);
        
        match category {
            ErrorCategory::Retryable { delay_seconds, .. } => {
                assert_eq!(delay_seconds, 30); // Longer delay for rate limits
            }
            _ => panic!("Expected Retryable"),
        }
    }
    
    #[test]
    fn test_classify_disk_full() {
        let error = anyhow::anyhow!("no space left on device");
        let category = ErrorClassifier::classify(&error);
        
        match category {
            ErrorCategory::Permanent { .. } => {}
            _ => panic!("Expected Permanent"),
        }
    }
    
    #[test]
    fn test_extract_http_status() {
        assert_eq!(ErrorClassifier::extract_http_status("http error: 404"), Some(404));
        assert_eq!(ErrorClassifier::extract_http_status("status code 503"), Some(503));
        assert_eq!(ErrorClassifier::extract_http_status("http 429 too many"), Some(429));
        assert_eq!(ErrorClassifier::extract_http_status("no status here"), None);
    }
}
