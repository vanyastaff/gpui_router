//! Nested route resolution
//!
//! This module provides functionality for resolving child routes in nested routing scenarios.
//! The cache functionality has been moved to the `cache` module (available with `cache` feature).

use crate::route::Route;
use crate::{trace_log, warn_log, RouteParams};
use std::borrow::Cow;
use std::sync::Arc;

/// Resolved child route information
///
/// Contains the matched child route and merged parameters from parent and child.
pub type ResolvedChildRoute = (Arc<Route>, RouteParams);

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
/// use gpui_router::build_child_path;
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
