//! Router state management

use crate::route::Route;
use crate::{NavigationDirection, RouteChangeEvent, RouteMatch};
use std::collections::HashMap;
use std::sync::Arc;

/// Router state
#[derive(Debug, Clone)]
pub struct RouterState {
    /// Navigation history stack
    history: Vec<String>,
    /// Current position in history
    current: usize,
    /// Registered routes
    routes: Vec<Arc<Route>>,
    /// Route cache
    cache: HashMap<String, RouteMatch>,
}

impl RouterState {
    /// Create a new router state
    pub fn new() -> Self {
        Self {
            history: vec!["/".to_string()],
            current: 0,
            routes: Vec::new(),
            cache: HashMap::new(),
        }
    }

    /// Register a route
    pub fn add_route(&mut self, route: Route) {
        self.routes.push(Arc::new(route));
        // Routes have changed, so any cached matches may now be stale.
        self.cache.clear();
    }

    /// Get current path
    pub fn current_path(&self) -> &str {
        &self.history[self.current]
    }

    /// Get all registered routes
    pub fn routes(&self) -> &[Arc<Route>] {
        &self.routes
    }

    /// Get current route match (with caching)
    pub fn current_match(&mut self) -> Option<RouteMatch> {
        let path = self.current_path();

        // Check cache first
        if let Some(cached) = self.cache.get(path) {
            return Some(cached.clone());
        }

        // Find matching route
        for route in &self.routes {
            if let Some(route_match) = route.matches(path) {
                self.cache.insert(path.to_string(), route_match.clone());
                return Some(route_match);
            }
        }

        None
    }

    /// Get current route match without caching (immutable)
    ///
    /// Use this when you need to access the current route from a non-mutable context,
    /// such as in a GPUI Render implementation.
    pub fn current_match_immutable(&self) -> Option<RouteMatch> {
        let path = self.current_path();

        // Check cache first
        if let Some(cached) = self.cache.get(path) {
            return Some(cached.clone());
        }

        // Find matching route without caching
        for route in &self.routes {
            if let Some(route_match) = route.matches(path) {
                return Some(route_match);
            }
        }

        None
    }

    /// Get the matched Route for current path
    ///
    /// Returns the Route object that matched, not just the RouteMatch.
    /// This is needed for rendering and accessing the route's builder.
    pub fn current_route(&self) -> Option<&Arc<Route>> {
        let path = self.current_path();

        self.routes
            .iter()
            .find(|route| route.matches(path).is_some())
    }

    /// Navigate to a new path
    pub fn push(&mut self, path: String) -> RouteChangeEvent {
        let from = Some(self.current_path().to_string());

        // Remove forward history when pushing
        self.history.truncate(self.current + 1);

        // Add new path
        self.history.push(path.clone());
        self.current += 1;

        RouteChangeEvent {
            from,
            to: path,
            direction: NavigationDirection::Forward,
        }
    }

    /// Replace current path
    pub fn replace(&mut self, path: String) -> RouteChangeEvent {
        let from = Some(self.current_path().to_string());

        self.history[self.current] = path.clone();

        RouteChangeEvent {
            from,
            to: path,
            direction: NavigationDirection::Replace,
        }
    }

    /// Go back in history
    pub fn back(&mut self) -> Option<RouteChangeEvent> {
        if self.current > 0 {
            let from = Some(self.current_path().to_string());
            self.current -= 1;
            let to = self.current_path().to_string();

            Some(RouteChangeEvent {
                from,
                to,
                direction: NavigationDirection::Back,
            })
        } else {
            None
        }
    }

    /// Go forward in history
    pub fn forward(&mut self) -> Option<RouteChangeEvent> {
        if self.current < self.history.len() - 1 {
            let from = Some(self.current_path().to_string());
            self.current += 1;
            let to = self.current_path().to_string();

            Some(RouteChangeEvent {
                from,
                to,
                direction: NavigationDirection::Forward,
            })
        } else {
            None
        }
    }

    /// Check if can go back
    pub fn can_go_back(&self) -> bool {
        self.current > 0
    }

    /// Check if can go forward
    pub fn can_go_forward(&self) -> bool {
        self.current < self.history.len() - 1
    }

    /// Clear navigation history
    pub fn clear(&mut self) {
        self.history.clear();
        self.history.push("/".to_string());
        self.current = 0;
        self.cache.clear();
    }
}

impl Default for RouterState {
    fn default() -> Self {
        Self::new()
    }
}

/// Router - manages navigation state
pub struct Router {
    state: RouterState,
}

impl Router {
    /// Create a new router
    pub fn new() -> Self {
        Self {
            state: RouterState::new(),
        }
    }

    /// Get mutable reference to state
    pub fn state_mut(&mut self) -> &mut RouterState {
        &mut self.state
    }

    /// Get reference to state
    pub fn state(&self) -> &RouterState {
        &self.state
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation() {
        let mut state = RouterState::new();

        assert_eq!(state.current_path(), "/");

        state.push("/users".to_string());
        assert_eq!(state.current_path(), "/users");

        state.push("/users/123".to_string());
        assert_eq!(state.current_path(), "/users/123");

        state.back();
        assert_eq!(state.current_path(), "/users");

        state.forward();
        assert_eq!(state.current_path(), "/users/123");
    }

    #[test]
    fn test_replace() {
        let mut state = RouterState::new();

        state.push("/users".to_string());
        state.replace("/posts".to_string());

        assert_eq!(state.current_path(), "/posts");
        assert_eq!(state.history.len(), 2);
    }
}
