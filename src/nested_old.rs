//! Route resolution caching for nested routes
//!
//! This module provides caching functionality to avoid repeated route lookups
//! during rendering. The cache stores the results of expensive operations like
//! parent route finding and child route resolution.
//!
//! ## Architecture (Arc routes)
//!
//! For production correctness and performance we avoid cloning `Route` (it contains
//! behavior like guards/middleware/lifecycle). Instead we pass around `Arc<Route>`
//! and cache lightweight identifiers / params for fast outlet resolution.

use crate::route::Route;
use crate::{trace_log, warn_log};
#[cfg(feature = "cache")]
use lru::LruCache;
use std::borrow::Cow;
#[cfg(feature = "cache")]
use std::num::NonZeroUsize;
use std::sync::Arc;

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
///
/// Outlets are identified by the current path and an optional name
/// (for named outlets like "sidebar", "modal", etc.)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OutletCacheKey {
    /// Current navigation path
    path: String,
    /// Optional outlet name (None for default outlet)
    outlet_name: Option<String>,
}

/// Cached result of finding a parent route
#[derive(Debug, Clone)]
struct ParentRouteCacheEntry {
    /// ID of the parent route (route with children)
    parent_route_id: RouteId,
}

/// Cached result of resolving a child route.
///
/// Stores the resolution result for quick lookup. The actual Route object
/// cannot be cached directly because it contains function pointers, but we
/// store enough information to validate cache hits and potentially reconstruct
/// the resolution.
#[derive(Debug, Clone)]
struct ChildRouteCacheEntry {
    /// ID of the resolved child route
    pub child_route_id: RouteId,
    /// Extracted route parameters
    pub params: crate::RouteParams,
    /// Remaining path after this child
    pub remaining_path: String,
}

impl ChildRouteCacheEntry {
    /// Check if this cache entry matches the given resolved child route.
    #[allow(dead_code)] // Used for cache validation in future implementations
    fn matches(&self, resolved: &crate::nested::ResolvedChildRoute) -> bool {
        // We no longer carry `remaining_path` in the public resolution result.
        // Cache validation is therefore limited to verifying the resolved route id.
        self.child_route_id.path == resolved.0.config.path
    }

    /// Get the cached route ID.
    #[allow(dead_code)] // Reserved for cache lookup optimization
    pub fn route_id(&self) -> &RouteId {
        &self.child_route_id
    }

    /// Get the cached parameters.
    #[allow(dead_code)] // Reserved for cache lookup optimization
    pub fn params(&self) -> &crate::RouteParams {
        &self.params
    }

    /// Get the remaining path.
    #[allow(dead_code)] // Reserved for cache lookup optimization
    pub fn remaining(&self) -> &str {
        &self.remaining_path
    }
}

/// Route resolution cache
///
/// Caches the results of expensive route lookups to avoid repeated
/// tree traversals during rendering.
///
/// # Cache Invalidation
///
/// The cache must be cleared when:
/// - Navigation occurs (path changes)
/// - Route configuration changes (routes added/removed)
/// - Manual invalidation requested
///
/// # Memory Usage
///
/// The cache stores lightweight identifiers (RouteId) rather than
/// full Route clones, keeping memory overhead minimal.
///
/// # Cache Limits
///
/// Uses LRU (Least Recently Used) eviction policy to prevent unbounded growth.
/// Default capacity: 1000 entries per cache.
#[derive(Debug)]
pub struct RouteCache {
    /// Cache for parent route lookups
    ///
    /// Key: current path
    /// Value: RouteId of the parent route with children
    parent_cache: LruCache<String, ParentRouteCacheEntry>,

    /// Cache for child route resolutions
    ///
    /// Key: (current path, outlet name)
    /// Value: Resolved child route info
    child_cache: LruCache<OutletCacheKey, ChildRouteCacheEntry>,

    /// Statistics for monitoring cache performance
    stats: CacheStats,
}

/// Cache performance statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits for parent lookups
    pub parent_hits: usize,
    /// Number of cache misses for parent lookups
    pub parent_misses: usize,
    /// Number of cache hits for child resolutions
    pub child_hits: usize,
    /// Number of cache misses for child resolutions
    pub child_misses: usize,
    /// Total number of cache invalidations
    pub invalidations: usize,
}

impl CacheStats {
    /// Calculate parent cache hit rate (0.0 to 1.0)
    pub fn parent_hit_rate(&self) -> f64 {
        let total = self.parent_hits + self.parent_misses;
        if total == 0 {
            0.0
        } else {
            self.parent_hits as f64 / total as f64
        }
    }

    /// Calculate child cache hit rate (0.0 to 1.0)
    pub fn child_hit_rate(&self) -> f64 {
        let total = self.child_hits + self.child_misses;
        if total == 0 {
            0.0
        } else {
            self.child_hits as f64 / total as f64
        }
    }

    /// Calculate overall hit rate
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

impl RouteCache {
    /// Default cache capacity per cache type
    const DEFAULT_CAPACITY: usize = 1000;

    /// Create a new empty cache with default capacity
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// Create a new cache with specified capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries per cache (parent and child caches separately)
    ///
    /// # Panics
    ///
    /// Panics if capacity is 0
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).expect("Cache capacity must be non-zero");
        Self {
            parent_cache: LruCache::new(cap),
            child_cache: LruCache::new(cap),
            stats: CacheStats::default(),
        }
    }

    /// Clear all cached entries
    ///
    /// Call this when:
    /// - Navigation occurs (path changes)
    /// - Route configuration changes
    pub fn clear(&mut self) {
        trace_log!("Clearing route cache");
        self.parent_cache.clear();
        self.child_cache.clear();
        self.stats.invalidations += 1;
    }

    /// Get cached parent route for a path
    ///
    /// Returns the RouteId of the parent route if cached.
    /// Using LruCache automatically promotes accessed entries.
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

    /// Cache a parent route lookup result
    ///
    /// If cache is full, evicts the least recently used entry.
    pub fn set_parent(&mut self, path: String, parent_route_id: RouteId) {
        trace_log!(
            "Caching parent route '{}' for path '{}'",
            parent_route_id.path,
            path
        );
        self.parent_cache
            .push(path, ParentRouteCacheEntry { parent_route_id });
    }

    /// Get cached child route resolution
    ///
    /// With the Arc-based route graph, we can return cached results safely.
    pub fn get_child(
        &mut self,
        path: &str,
        outlet_name: Option<&str>,
    ) -> Option<ResolvedChildRoute> {
        let key = OutletCacheKey {
            path: path.to_string(),
            outlet_name: outlet_name.map(|s| s.to_string()),
        };

        if let Some(entry) = self.child_cache.get(&key) {
            self.stats.child_hits += 1;
            trace_log!(
                "Child cache hit for path: '{}', outlet: {:?}",
                path,
                outlet_name
            );

            // We currently don't reconstruct `Arc<Route>` from `RouteId` here because
            // that requires a route index/lookup table. We still return merged params
            // if/when reconstruction becomes available.
            //
            // For now, treat this cache as a placeholder and return None.
            let _ = entry;
            None
        } else {
            self.stats.child_misses += 1;
            trace_log!(
                "Child cache miss for path: '{}', outlet: {:?}",
                path,
                outlet_name
            );
            None
        }
    }

    /// Cache a child route resolution result
    ///
    /// If cache is full, evicts the least recently used entry.
    pub fn set_child(
        &mut self,
        path: String,
        outlet_name: Option<String>,
        resolved: &ResolvedChildRoute,
    ) {
        let key = OutletCacheKey {
            path: path.clone(),
            outlet_name: outlet_name.clone(),
        };

        trace_log!(
            "Caching child route '{}' for path '{}', outlet: {:?}",
            resolved.0.config.path,
            path,
            outlet_name
        );

        self.child_cache.push(
            key,
            ChildRouteCacheEntry {
                child_route_id: RouteId::from_route(resolved.0.as_ref()),
                params: resolved.1.clone(),
                // `ResolvedChildRoute` no longer carries remaining_path; keep empty for now.
                remaining_path: String::new(),
            },
        );
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Reset statistics (keeps cached data)
    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }

    /// Get number of cached parent entries
    pub fn parent_cache_size(&self) -> usize {
        self.parent_cache.len()
    }

    /// Get number of cached child entries
    pub fn child_cache_size(&self) -> usize {
        self.child_cache.len()
    }

    /// Get total cache size (number of entries)
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
        // Create new cache with same capacity
        let parent_cap = self.parent_cache.cap();
        let child_cap = self.child_cache.cap();

        // Note: LruCache doesn't provide iter(), so we create fresh caches
        // This is acceptable since cache is meant to be ephemeral and invalidated on navigation
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
    use gpui::{div, IntoElement, ParentElement};

    #[test]
    fn test_cache_creation() {
        let cache = RouteCache::new();
        assert_eq!(cache.parent_cache_size(), 0);
        assert_eq!(cache.child_cache_size(), 0);
        assert_eq!(cache.stats().parent_hits, 0);
    }

    #[test]
    fn test_parent_cache_miss() {
        let mut cache = RouteCache::new();
        let result = cache.get_parent("/dashboard");
        assert!(result.is_none());
        assert_eq!(cache.stats().parent_misses, 1);
        assert_eq!(cache.stats().parent_hits, 0);
    }

    #[test]
    fn test_parent_cache_hit() {
        let mut cache = RouteCache::new();
        let route_id = RouteId::from_path("/dashboard");

        // First access - miss
        cache.set_parent("/dashboard/analytics".to_string(), route_id.clone());

        // Second access - hit
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

        // 3 misses
        cache.get_parent("/a");
        cache.get_parent("/b");
        cache.get_parent("/c");

        // Cache some entries
        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        cache.set_parent("/b".to_string(), RouteId::from_path("/"));

        // 2 hits
        cache.get_parent("/a");
        cache.get_parent("/b");

        // Hit rate: 2 hits / 5 total = 0.4
        assert_eq!(cache.stats().parent_hits, 2);
        assert_eq!(cache.stats().parent_misses, 3);
        assert!((cache.stats().parent_hit_rate() - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_route_id_equality() {
        let id1 = RouteId::from_path("/dashboard");
        let id2 = RouteId::from_path("/dashboard");
        let id3 = RouteId::from_path("/settings");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_cache_stats_reset() {
        let mut cache = RouteCache::new();

        cache.get_parent("/a"); // miss
        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        cache.get_parent("/a"); // hit

        assert_eq!(cache.stats().parent_hits, 1);

        cache.reset_stats();

        assert_eq!(cache.stats().parent_hits, 0);
        assert_eq!(cache.stats().parent_misses, 0);
        // But cache data is preserved
        assert_eq!(cache.parent_cache_size(), 1);
    }

    // Child route resolver tests

    #[test]
    fn test_resolve_dashboard_analytics() {
        // Create dashboard route with analytics child
        let dashboard = Arc::new(
            Route::new("dashboard", |_, _cx, _params| {
                div().child("Dashboard").into_any_element()
            })
            .children(vec![Arc::new(Route::new("analytics", |_, _cx, _params| {
                div().child("Analytics").into_any_element()
            }))]),
        );

        let params = RouteParams::new();
        let result = resolve_child_route(&dashboard, "/dashboard/analytics", &params, None);

        println!("\n=== TEST RESULT ===");
        println!("Resolving '/dashboard/analytics' under dashboard route");
        println!("Result: {:?}", result.is_some());

        if let Some((route, _params)) = &result {
            println!("Found child: '{}'", route.config.path);
        } else {
            println!("NO MATCH!");
        }

        assert!(result.is_some(), "Should find analytics child");
        assert_eq!(result.unwrap().0.config.path, "analytics");
    }

    #[test]
    fn test_build_child_path() {
        assert_eq!(
            build_child_path("/dashboard", "settings"),
            "/dashboard/settings"
        );
        assert_eq!(
            build_child_path("/dashboard/", "/settings"),
            "/dashboard/settings"
        );
        assert_eq!(build_child_path("/dashboard", ""), "/dashboard");
        assert_eq!(build_child_path("/", "home"), "/home");
        assert_eq!(build_child_path("", "home"), "/home");
    }

    #[test]
    fn test_path_trimming() {
        let parent = "/dashboard/";
        let child = "/settings/";
        let result = build_child_path(parent, child);
        assert_eq!(result, "/dashboard/settings");
    }

    #[test]
    fn test_resolve_simple_child() {
        // Create parent route with children
        let parent = Arc::new(
            Route::new("/dashboard", |_, _cx, _params| {
                div().child("Dashboard").into_any_element()
            })
            .children(vec![
                Arc::new(Route::new("overview", |_, _cx, _params| {
                    div().child("Overview").into_any_element()
                })),
                Arc::new(Route::new("settings", |_, _cx, _params| {
                    div().child("Settings").into_any_element()
                })),
            ]),
        );

        // Test resolving "settings" child
        let result = resolve_child_route(&parent, "/dashboard/settings", &RouteParams::new(), None);

        assert!(result.is_some());
        let (child_route, _params) = result.unwrap();
        assert_eq!(child_route.config.path, "settings");
    }

    #[test]
    fn test_resolve_index_route() {
        // Create parent with index route
        let parent = Arc::new(
            Route::new("/dashboard", |_, _cx, _params| {
                div().child("Dashboard").into_any_element()
            })
            .children(vec![Arc::new(Route::new("", |_, _cx, _params| {
                div().child("Index").into_any_element()
            }))]),
        );

        // Test resolving index when path is just parent
        let result = resolve_child_route(&parent, "/dashboard", &RouteParams::new(), None);

        assert!(result.is_some());
        let (child_route, _params) = result.unwrap();
        assert_eq!(child_route.config.path, "");
    }

    #[test]
    fn test_no_matching_child() {
        let parent = Arc::new(
            Route::new("/dashboard", |_, _cx, _params| {
                div().child("Dashboard").into_any_element()
            })
            .children(vec![Arc::new(Route::new("overview", |_, _cx, _params| {
                div().child("Overview").into_any_element()
            }))]),
        );

        // Test with non-existent child path
        let result =
            resolve_child_route(&parent, "/dashboard/nonexistent", &RouteParams::new(), None);

        assert!(result.is_none());
    }

    #[test]
    fn test_named_outlet_resolution() {
        // Create parent with named outlet
        let parent = Arc::new(
            Route::new("/dashboard", |_, _cx, _params| {
                div().child("Dashboard").into_any_element()
            })
            .children(vec![
                // Default outlet children
                Arc::new(Route::new("main", |_, _cx, _params| {
                    div().child("Main Content").into_any_element()
                })),
            ])
            .named_outlet(
                "sidebar",
                vec![
                    // Sidebar outlet children
                    Arc::new(Route::new("main", |_, _cx, _params| {
                        div().child("Sidebar Content").into_any_element()
                    })),
                ],
            ),
        );

        // Test default outlet
        let result = resolve_child_route(&parent, "/dashboard/main", &RouteParams::new(), None);
        assert!(result.is_some());

        // Test named outlet
        let result = resolve_child_route(
            &parent,
            "/dashboard/main",
            &RouteParams::new(),
            Some("sidebar"),
        );
        assert!(result.is_some());
        let (child_route, _params) = result.unwrap();
        assert_eq!(child_route.config.path, "main");
    }

    #[test]
    fn test_named_outlet_not_found() {
        let parent = Arc::new(
            Route::new("/dashboard", |_, _cx, _params| {
                div().child("Dashboard").into_any_element()
            })
            .children(vec![Arc::new(Route::new("main", |_, _cx, _params| {
                div().child("Main").into_any_element()
            }))]),
        );

        // Test non-existent named outlet
        let result = resolve_child_route(
            &parent,
            "/dashboard/main",
            &RouteParams::new(),
            Some("sidebar"),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_multiple_named_outlets() {
        let parent = Arc::new(
            Route::new("/app", |_, _cx, _params| div().child("App").into_any_element())
                .children(vec![Arc::new(Route::new("page", |_, _cx, _params| {
                    div().child("Main Page").into_any_element()
                }))])
                .named_outlet(
                    "sidebar",
                    vec![Arc::new(Route::new("page", |_, _cx, _params| {
                        div().child("Sidebar").into_any_element()
                    }))],
                )
                .named_outlet(
                    "footer",
                    vec![Arc::new(Route::new("page", |_, _cx, _params| {
                        div().child("Footer").into_any_element()
                    }))],
                ),
        );

        // Test all three outlets resolve correctly
        let main_result = resolve_child_route(&parent, "/app/page", &RouteParams::new(), None);
        assert!(main_result.is_some());

        let sidebar_result =
            resolve_child_route(&parent, "/app/page", &RouteParams::new(), Some("sidebar"));
        assert!(sidebar_result.is_some());

        let footer_result =
            resolve_child_route(&parent, "/app/page", &RouteParams::new(), Some("footer"));
        assert!(footer_result.is_some());
    }

    #[test]
    fn test_named_outlet_with_index_route() {
        let parent = Arc::new(
            Route::new("/dashboard", |_, _cx, _params| {
                div().child("Dashboard").into_any_element()
            })
            .children(vec![Arc::new(Route::new("", |_, _cx, _params| {
                div().child("Main Index").into_any_element()
            }))])
            .named_outlet(
                "sidebar",
                vec![Arc::new(Route::new("", |_, _cx, _params| {
                    div().child("Sidebar Index").into_any_element()
                }))],
            ),
        );

        // Test index routes in both outlets
        let main_result = resolve_child_route(&parent, "/dashboard", &RouteParams::new(), None);
        assert!(main_result.is_some());

        let sidebar_result =
            resolve_child_route(&parent, "/dashboard", &RouteParams::new(), Some("sidebar"));
        assert!(sidebar_result.is_some());
    }
}

// ============================================================================
// Child Route Resolver
// ============================================================================
//
// This section handles resolving which child route should be rendered
// in a RouterOutlet based on the current navigation state.

use crate::RouteParams;

/// Result of resolving a child route for an outlet.
///
/// We keep this lightweight and avoid extra structs:
/// - `Arc<Route>`: the matched child route
/// - `RouteParams`: combined params from parent + child
pub type ResolvedChildRoute = (Arc<Route>, RouteParams);

/// Resolve which child route should be rendered for the current path
///
/// This function takes a parent route and the current path, and determines
/// which child route (if any) should be rendered in the outlet.
///
/// # Arguments
///
/// * `parent_route` - The parent route containing children
/// * `current_path` - The current navigation path
/// * `parent_params` - Parameters extracted from the parent route
/// * `outlet_name` - Optional name of the outlet (for named outlets)
///
/// # Returns
///
/// `Some(ChildRouteInfo)` if a matching child route is found, `None` otherwise
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::{Route, RouteParams, resolve_child_route};
///
/// let parent = Route::new("/dashboard", |_, _cx, _params| gpui::div().into_any_element())
///     .children(vec![
///         Route::new("overview", |_, _cx, _params| gpui::div().into_any_element()),
///         Route::new("settings", |_, _cx, _params| gpui::div().into_any_element()),
///     ]);
///
/// // When navigating to "/dashboard/settings"
/// let child_info = resolve_child_route(
///     &parent,
///     "/dashboard/settings",
///     &RouteParams::new(),
///     None
/// );
/// ```
pub fn resolve_child_route(
    parent_route: &Arc<Route>,
    current_path: &str,
    parent_params: &RouteParams,
    outlet_name: Option<&str>,
) -> Option<ResolvedChildRoute> {
    trace_log!(
        "resolve_child_route: parent='{}', current_path='{}', children={}, outlet_name={:?}",
        parent_route.config.path,
        current_path,
        parent_route.get_children().len(),
        outlet_name
    );

    // Get the children for this outlet (named or default)
    let children = if let Some(name) = outlet_name {
        // Named outlet - get children from named_children map
        match parent_route.get_named_children(name) {
            Some(named_children) => {
                trace_log!(
                    "Using named outlet '{}' with {} children",
                    name,
                    named_children.len()
                );
                named_children
            }
            None => {
                warn_log!(
                    "Named outlet '{}' not found in route '{}'",
                    name,
                    parent_route.config.path
                );
                return None;
            }
        }
    } else {
        // Default outlet - use regular children
        parent_route.get_children()
    };

    if children.is_empty() {
        trace_log!("No children found for outlet {:?}", outlet_name);
        return None;
    }

    // Extract parent path from the route config
    let parent_path = &parent_route.config.path;

    // Normalize paths for comparison
    let parent_path_normalized = parent_path.trim_end_matches('/');
    let current_path_normalized = current_path.trim_start_matches('/');

    // Check if current path starts with parent path
    if !current_path_normalized.starts_with(parent_path_normalized.trim_start_matches('/')) {
        return None;
    }

    // Get the remaining path after parent
    let remaining = if parent_path_normalized.is_empty() || parent_path_normalized == "/" {
        current_path_normalized
    } else {
        current_path_normalized
            .strip_prefix(parent_path_normalized.trim_start_matches('/'))
            .unwrap_or("")
            .trim_start_matches('/')
    };

    trace_log!(
        "  normalized: parent='{}', current='{}', remaining='{}'",
        parent_path_normalized,
        current_path_normalized,
        remaining
    );

    if remaining.is_empty() {
        // No child path, look for index route
        return find_index_route(children, parent_params.clone());
    }

    // Split remaining path into segments
    let segments: Vec<&str> = remaining.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return find_index_route(children, parent_params.clone());
    }

    let first_segment = segments[0];
    trace_log!("  first_segment: '{}'", first_segment);

    // Try to match first segment against child routes
    for child in children {
        let child_path = child.config.path.trim_start_matches('/');

        // Check for exact match or parameter match
        if child_path == first_segment || child_path.starts_with(':') {
            trace_log!("  matched: '{}'", child_path);
            // Found matching child!
            let mut combined_params = parent_params.clone();

            // If this is a parameter route, extract the parameter
            if child_path.starts_with(':') {
                let param_name = child_path.trim_start_matches(':');
                combined_params.insert(param_name.to_string(), first_segment.to_string());
            }

            // TODO: Handle nested parameters in deeper child paths

            return Some((Arc::clone(child), combined_params));
        }
    }

    None
}

/// Find an index route (default child route when no specific child is selected)
fn find_index_route(children: &[Arc<Route>], params: RouteParams) -> Option<ResolvedChildRoute> {
    // Look for a child with empty path, "/" or "index"
    for child in children {
        let child_path = child.config.path.trim_start_matches('/');

        if child_path.is_empty() || child_path == "/" || child_path == "index" {
            return Some((Arc::clone(child), params));
        }
    }

    None
}

/// Build the full path for a child route
///
/// Combines parent and child paths into a complete route path.
///
/// Returns `Cow<str>` to avoid unnecessary allocations when possible.
/// Uses borrowed string when no modification is needed.
///
/// # Example
///
/// ```
/// use gpui_navigator::build_child_path;
///
/// let full_path = build_child_path("/dashboard", "settings");
/// assert_eq!(full_path, "/dashboard/settings");
/// ```
pub fn build_child_path<'a>(parent_path: &'a str, child_path: &'a str) -> Cow<'a, str> {
    let parent = parent_path.trim_end_matches('/');
    let child = child_path.trim_start_matches('/').trim_end_matches('/');

    if child.is_empty() {
        // Return parent as-is if child is empty (avoid allocation)
        if parent == parent_path {
            Cow::Borrowed(parent_path)
        } else {
            Cow::Owned(parent.to_string())
        }
    } else if parent.is_empty() || parent == "/" {
        Cow::Owned(format!("/{}", child))
    } else {
        Cow::Owned(format!("{}/{}", parent, child))
    }
}
