//! Route resolution caching
//!
//! This module provides caching functionality to avoid repeated route lookups
//! during rendering with LRU eviction policy.

use crate::route::Route;
use crate::{trace_log, RouteParams};
use lru::LruCache;
use std::num::NonZeroUsize;

/// Unique identifier for a route in the tree
///
/// This allows us to reference routes without storing full Route clones.
/// Routes are identified by their path hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteId {
    /// Full path of the route (e.g., "/dashboard/analytics")
    pub path: String,
}

impl RouteId {
    /// Create a new route ID from a route
    pub fn from_route(route: &Route) -> Self {
        Self {
            path: route.config.path.clone(),
        }
    }

    /// Create a route ID from a path string
    pub fn from_path(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

/// Cache key for outlet resolution
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OutletCacheKey {
    path: String,
    outlet_name: Option<String>,
}

/// Cached result of finding a parent route
#[derive(Debug, Clone)]
struct ParentRouteCacheEntry {
    parent_route_id: RouteId,
}

/// Cache performance statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub parent_hits: usize,
    pub parent_misses: usize,
    pub child_hits: usize,
    pub child_misses: usize,
    pub invalidations: usize,
}

impl CacheStats {
    pub fn parent_hit_rate(&self) -> f64 {
        let total = self.parent_hits + self.parent_misses;
        if total == 0 {
            0.0
        } else {
            self.parent_hits as f64 / total as f64
        }
    }

    pub fn child_hit_rate(&self) -> f64 {
        let total = self.child_hits + self.child_misses;
        if total == 0 {
            0.0
        } else {
            self.child_hits as f64 / total as f64
        }
    }

    pub fn overall_hit_rate(&self) -> f64 {
        let total_hits = self.parent_hits + self.child_hits;
        let total_misses = self.parent_misses + self.child_misses;
        let total = total_hits + total_misses;
        if total == 0 {
            0.0
        } else {
            total_hits as f64 / total as f64
        }
    }
}

/// Route resolution cache with LRU eviction
///
/// Default capacity: 1000 entries per cache.
#[derive(Debug)]
pub struct RouteCache {
    parent_cache: LruCache<String, ParentRouteCacheEntry>,
    child_cache: LruCache<OutletCacheKey, RouteParams>,
    stats: CacheStats,
}

impl RouteCache {
    const DEFAULT_CAPACITY: usize = 1000;

    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).expect("Cache capacity must be non-zero");
        Self {
            parent_cache: LruCache::new(cap),
            child_cache: LruCache::new(cap),
            stats: CacheStats::default(),
        }
    }

    pub fn clear(&mut self) {
        trace_log!("Clearing route cache");
        self.parent_cache.clear();
        self.child_cache.clear();
        self.stats.invalidations += 1;
    }

    pub fn get_parent(&mut self, path: &str) -> Option<RouteId> {
        if let Some(entry) = self.parent_cache.get(path) {
            self.stats.parent_hits += 1;
            trace_log!("Parent cache hit for path: '{}'", path);
            Some(entry.parent_route_id.clone())
        } else {
            self.stats.parent_misses += 1;
            trace_log!("Parent cache miss for path: '{}'", path);
            None
        }
    }

    pub fn set_parent(&mut self, path: String, parent_route_id: RouteId) {
        trace_log!(
            "Caching parent route '{}' for path '{}'",
            parent_route_id.path,
            path
        );
        self.parent_cache
            .push(path, ParentRouteCacheEntry { parent_route_id });
    }

    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }

    pub fn parent_cache_size(&self) -> usize {
        self.parent_cache.len()
    }

    pub fn child_cache_size(&self) -> usize {
        self.child_cache.len()
    }

    pub fn total_size(&self) -> usize {
        self.parent_cache_size() + self.child_cache_size()
    }
}

impl Default for RouteCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for RouteCache {
    fn clone(&self) -> Self {
        let parent_cap = self.parent_cache.cap();
        let child_cap = self.child_cache.cap();
        Self {
            parent_cache: LruCache::new(parent_cap),
            child_cache: LruCache::new(child_cap),
            stats: self.stats.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = RouteCache::new();
        assert_eq!(cache.parent_cache_size(), 0);
        assert_eq!(cache.stats().parent_hits, 0);
    }

    #[test]
    fn test_parent_cache_miss() {
        let mut cache = RouteCache::new();
        let result = cache.get_parent("/dashboard");
        assert!(result.is_none());
        assert_eq!(cache.stats().parent_misses, 1);
    }

    #[test]
    fn test_parent_cache_hit() {
        let mut cache = RouteCache::new();
        let route_id = RouteId::from_path("/dashboard");
        cache.set_parent("/dashboard/analytics".to_string(), route_id.clone());

        let result = cache.get_parent("/dashboard/analytics");
        assert!(result.is_some());
        assert_eq!(result.unwrap().path, "/dashboard");
        assert_eq!(cache.stats().parent_hits, 1);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = RouteCache::new();
        cache.set_parent("/dashboard".to_string(), RouteId::from_path("/"));
        assert_eq!(cache.parent_cache_size(), 1);

        cache.clear();
        assert_eq!(cache.parent_cache_size(), 0);
        assert_eq!(cache.stats().invalidations, 1);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let mut cache = RouteCache::new();
        cache.get_parent("/a");
        cache.get_parent("/b");
        cache.get_parent("/c");

        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        cache.set_parent("/b".to_string(), RouteId::from_path("/"));

        cache.get_parent("/a");
        cache.get_parent("/b");

        assert_eq!(cache.stats().parent_hits, 2);
        assert_eq!(cache.stats().parent_misses, 3);
        assert!((cache.stats().parent_hit_rate() - 0.4).abs() < 0.001);
    }
}
