//! HostRegistry - Plugin manager for file hosts

use std::collections::HashMap;
use super::base::HostHandler;

/// Host registry - manages all host handlers
pub struct HostRegistry {
    handlers: HashMap<String, Box<dyn HostHandler>>,
}

impl HostRegistry {
    /// Create new empty registry
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
    
    /// Register a host handler
    pub fn register(&mut self, handler: Box<dyn HostHandler>) {
        let name = handler.get_host_name().to_string();
        self.handlers.insert(name, handler);
    }
    
    /// Get handler for a URL
    pub fn get_handler_for_url(&self, url: &str) -> Option<&dyn HostHandler> {
        for handler in self.handlers.values() {
            if handler.can_handle(url) {
                return Some(handler.as_ref());
            }
        }
        None
    }
    
    /// Get handler by name
    pub fn get_handler(&self, name: &str) -> Option<&dyn HostHandler> {
        self.handlers.get(name).map(|h| h.as_ref())
    }
    
    /// List all registered handler names
    pub fn list_handlers(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for HostRegistry {
    fn default() -> Self {
        Self::new()
    }
}
