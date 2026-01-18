//! Advanced route matching with priority system
//!
//! This module provides a more sophisticated route matching system
//! compared to the simple pattern matching in route.rs.
//!
//! Features:
//! - Priority-based route matching (specific routes before generic)
//! - Optional segments support
//! - Constraint validation
//! - Better performance with early exit

use std::collections::HashMap;

/// Route path representation
#[derive(Debug, Clone, PartialEq)]
pub enum RoutePath {
    /// Static path like "/users"
    Static(&'static str),
    /// Dynamic path with string pattern
    Dynamic(String),
    /// Complex pattern with segments
    Pattern(RoutePattern),
}

/// A complete route pattern with segments and priority
#[derive(Debug, Clone, PartialEq)]
pub struct RoutePattern {
    /// Pattern segments
    pub segments: Vec<Segment>,
    /// Matching priority (higher = matched first)
    /// Calculated based on segment types
    pub priority: u8,
}

impl RoutePattern {
    /// Create a new route pattern from path string
    ///
    /// Examples:
    /// - "/users" -> static segments, priority 100
    /// - "/users/:id" -> mixed segments, priority 50
    /// - "/files/*" -> wildcard, priority 10
    pub fn from_path(path: &str) -> Self {
        let segments: Vec<Segment> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(Segment::parse)
            .collect();

        let priority = Self::calculate_priority(&segments);

        Self { segments, priority }
    }

    /// Calculate priority based on segment types
    ///
    /// Priority rules:
    /// - All static segments: 100
    /// - Each dynamic segment: -10
    /// - Optional segment: -5
    /// - Wildcard: 0
    fn calculate_priority(segments: &[Segment]) -> u8 {
        let mut priority: u8 = 100;

        for segment in segments {
            match segment {
                Segment::Static(_) => {
                    // Static segments don't reduce priority
                }
                Segment::Param { .. } => {
                    priority = priority.saturating_sub(10);
                }
                Segment::Optional(_) => {
                    priority = priority.saturating_sub(5);
                }
                Segment::Wildcard => {
                    // Wildcards have lowest priority
                    return 0;
                }
            }
        }

        priority
    }

    /// Match this pattern against a path
    ///
    /// Returns extracted parameters if matched
    pub fn matches(&self, path: &str) -> Option<HashMap<String, String>> {
        let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        self.match_segments(&path_segments)
    }

    /// Match segments against path segments
    fn match_segments(&self, path_segments: &[&str]) -> Option<HashMap<String, String>> {
        let mut params = HashMap::new();
        let mut path_idx = 0;
        let mut pattern_idx = 0;

        while pattern_idx < self.segments.len() {
            let segment = &self.segments[pattern_idx];

            match segment {
                Segment::Static(expected) => {
                    // Must match exactly
                    if path_idx >= path_segments.len() || path_segments[path_idx] != expected {
                        return None;
                    }
                    path_idx += 1;
                }
                Segment::Param { name, constraint } => {
                    // Extract parameter
                    if path_idx >= path_segments.len() {
                        return None;
                    }

                    let value = path_segments[path_idx];

                    // Validate constraint if present
                    if let Some(constraint) = constraint {
                        if !constraint.validate(value) {
                            return None;
                        }
                    }

                    params.insert(name.clone(), value.to_string());
                    path_idx += 1;
                }
                Segment::Optional(inner) => {
                    // Try to match, but don't fail if it doesn't
                    if path_idx < path_segments.len() {
                        if let Segment::Param { name, constraint } = &**inner {
                            let value = path_segments[path_idx];

                            let is_valid = if let Some(constraint) = constraint {
                                constraint.validate(value)
                            } else {
                                true
                            };

                            if is_valid {
                                params.insert(name.clone(), value.to_string());
                                path_idx += 1;
                            }
                        }
                    }
                }
                Segment::Wildcard => {
                    // Wildcard matches rest of path - always succeeds
                    return Some(params);
                }
            }

            pattern_idx += 1;
        }

        // All segments matched - check that we consumed all path segments
        if path_idx == path_segments.len() {
            Some(params)
        } else {
            None
        }
    }
}

/// A single segment in a route pattern
#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    /// Static text that must match exactly
    Static(String),
    /// Parameter that captures a value
    Param {
        name: String,
        constraint: Option<Constraint>,
    },
    /// Optional segment (can be missing)
    Optional(Box<Segment>),
    /// Wildcard that matches everything
    Wildcard,
}

impl Segment {
    /// Parse a segment from string
    ///
    /// Examples:
    /// - "users" -> Static("users")
    /// - ":id" -> Param { name: "id", constraint: None }
    /// - ":id<\\d+>" -> Param { name: "id", constraint: Some(Regex) }
    /// - "*" -> Wildcard
    pub fn parse(s: &str) -> Self {
        if s == "*" {
            return Segment::Wildcard;
        }

        if let Some(rest) = s.strip_prefix(':') {
            // Parameter segment

            // Check for constraint: :id<\d+>
            if let Some(pos) = rest.find('<') {
                let name = rest[..pos].to_string();
                let constraint_str = &rest[pos + 1..rest.len() - 1]; // Remove < and >

                let constraint = Constraint::parse(constraint_str);

                Segment::Param {
                    name,
                    constraint: Some(constraint),
                }
            } else {
                Segment::Param {
                    name: rest.to_string(),
                    constraint: None,
                }
            }
        } else {
            // Static segment
            Segment::Static(s.to_string())
        }
    }
}

/// Constraint for validating parameter values
#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    /// Regex pattern (simple implementation for now)
    Pattern(String),
    /// Numeric constraint
    Numeric,
    /// UUID constraint
    Uuid,
}

impl Constraint {
    /// Parse constraint from string
    fn parse(s: &str) -> Self {
        match s {
            "\\d+" => Constraint::Numeric,
            "uuid" => Constraint::Uuid,
            _ => Constraint::Pattern(s.to_string()),
        }
    }

    /// Validate a value against this constraint
    pub fn validate(&self, value: &str) -> bool {
        match self {
            Constraint::Numeric => value.chars().all(|c| c.is_ascii_digit()),
            Constraint::Uuid => {
                // Simple UUID validation: 8-4-4-4-12 hex chars
                let parts: Vec<&str> = value.split('-').collect();
                if parts.len() != 5 {
                    return false;
                }

                parts[0].len() == 8
                    && parts[1].len() == 4
                    && parts[2].len() == 4
                    && parts[3].len() == 4
                    && parts[4].len() == 12
                    && parts
                        .iter()
                        .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
            }
            Constraint::Pattern(_pattern) => {
                // TODO: Implement regex matching
                // For now, accept everything
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_parsing() {
        assert_eq!(
            Segment::parse("users"),
            Segment::Static("users".to_string())
        );
        assert_eq!(
            Segment::parse(":id"),
            Segment::Param {
                name: "id".to_string(),
                constraint: None
            }
        );
        assert_eq!(Segment::parse("*"), Segment::Wildcard);
    }

    #[test]
    fn test_segment_parsing_with_constraint() {
        let segment = Segment::parse(":id<\\d+>");
        match segment {
            Segment::Param { name, constraint } => {
                assert_eq!(name, "id");
                assert_eq!(constraint, Some(Constraint::Numeric));
            }
            _ => panic!("Expected Param segment"),
        }
    }

    #[test]
    fn test_priority_calculation() {
        let pattern1 = RoutePattern::from_path("/users");
        assert_eq!(pattern1.priority, 100); // All static

        let pattern2 = RoutePattern::from_path("/users/:id");
        assert_eq!(pattern2.priority, 90); // One dynamic

        let pattern3 = RoutePattern::from_path("/users/:id/posts/:postId");
        assert_eq!(pattern3.priority, 80); // Two dynamic

        let pattern4 = RoutePattern::from_path("/files/*");
        assert_eq!(pattern4.priority, 0); // Wildcard
    }

    #[test]
    fn test_static_route_matching() {
        let pattern = RoutePattern::from_path("/users");

        assert!(pattern.matches("/users").is_some());
        assert!(pattern.matches("/posts").is_none());
        assert!(pattern.matches("/users/123").is_none());
    }

    #[test]
    fn test_dynamic_route_matching() {
        let pattern = RoutePattern::from_path("/users/:id");

        let params = pattern.matches("/users/123");
        assert!(params.is_some());
        assert_eq!(params.unwrap().get("id"), Some(&"123".to_string()));

        assert!(pattern.matches("/users").is_none());
        assert!(pattern.matches("/users/123/posts").is_none());
    }

    #[test]
    fn test_wildcard_matching() {
        let pattern = RoutePattern::from_path("/files/*");

        assert!(pattern.matches("/files/docs").is_some());
        assert!(pattern.matches("/files/docs/report.pdf").is_some());
        assert!(pattern.matches("/other").is_none());
    }

    #[test]
    fn test_numeric_constraint() {
        let constraint = Constraint::Numeric;

        assert!(constraint.validate("123"));
        assert!(constraint.validate("0"));
        assert!(!constraint.validate("abc"));
        assert!(!constraint.validate("12a"));
    }

    #[test]
    fn test_uuid_constraint() {
        let constraint = Constraint::Uuid;

        assert!(constraint.validate("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!constraint.validate("not-a-uuid"));
        assert!(!constraint.validate("550e8400-e29b-41d4-a716"));
    }

    #[test]
    fn test_constrained_param_matching() {
        let pattern = RoutePattern::from_path("/users/:id<\\d+>");

        assert!(pattern.matches("/users/123").is_some());
        assert!(pattern.matches("/users/abc").is_none());
    }

    #[test]
    fn test_complex_pattern() {
        let pattern = RoutePattern::from_path("/api/users/:userId/posts/:postId");

        let params = pattern.matches("/api/users/42/posts/7");
        assert!(params.is_some());

        let params = params.unwrap();
        assert_eq!(params.get("userId"), Some(&"42".to_string()));
        assert_eq!(params.get("postId"), Some(&"7".to_string()));
    }
}
