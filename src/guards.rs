//! Route guards for authentication, authorization, and validation
//!
//! Guards are middleware that can block, allow, or redirect navigation.
//! They're useful for authentication, authorization, and validation.

use crate::{NavigationRequest, RouteMatch};
use gpui::App;
use std::future::Future;
use std::pin::Pin;

/// Result of a guard check
#[derive(Debug, Clone, PartialEq)]
pub enum GuardResult {
    /// Allow navigation to proceed
    Allow,

    /// Deny navigation with a reason
    Deny {
        /// Reason for denying navigation
        reason: String,
    },

    /// Redirect to a different path
    Redirect {
        /// Path to redirect to
        to: String,
        /// Reason for redirect (optional)
        reason: Option<String>,
    },
}

impl GuardResult {
    /// Create an allow result
    pub fn allow() -> Self {
        GuardResult::Allow
    }

    /// Create a deny result with reason
    pub fn deny(reason: impl Into<String>) -> Self {
        GuardResult::Deny {
            reason: reason.into(),
        }
    }

    /// Create a redirect result
    pub fn redirect(to: impl Into<String>) -> Self {
        GuardResult::Redirect {
            to: to.into(),
            reason: None,
        }
    }

    /// Create a redirect result with reason
    pub fn redirect_with_reason(to: impl Into<String>, reason: impl Into<String>) -> Self {
        GuardResult::Redirect {
            to: to.into(),
            reason: Some(reason.into()),
        }
    }

    /// Check if result is allow
    pub fn is_allow(&self) -> bool {
        matches!(self, GuardResult::Allow)
    }

    /// Check if result is deny
    pub fn is_deny(&self) -> bool {
        matches!(self, GuardResult::Deny { .. })
    }

    /// Check if result is redirect
    pub fn is_redirect(&self) -> bool {
        matches!(self, GuardResult::Redirect { .. })
    }

    /// Get redirect path if this is a redirect
    pub fn redirect_path(&self) -> Option<&str> {
        match self {
            GuardResult::Redirect { to, .. } => Some(to.as_str()),
            _ => None,
        }
    }
}

/// Trait for route guards
///
/// Guards can block navigation, allow it, or redirect to a different path.
/// Guards use an associated `Future` type for async operations without boxing.
///
/// # Design Benefits
///
/// - Zero-cost: No `Box<dyn Future>` allocation for concrete types
/// - Compile-time: Types checked at compile time
/// - Composable: Can be wrapped and combined
/// - Trait object safe: Can still use `Box<dyn RouteGuard>` when needed
///
/// # Example
///
/// ```no_run
/// use gpui_navigator::{RouteGuard, GuardResult, NavigationRequest};
/// use std::future::Future;
/// use std::pin::Pin;
///
/// struct MyAuthGuard {
///     redirect_to: String,
/// }
///
/// impl RouteGuard for MyAuthGuard {
///     type Future = Pin<Box<dyn Future<Output = GuardResult> + Send>>;
///
///     fn check(&self, _cx: &gpui::App, _request: &NavigationRequest) -> Self::Future {
///         let redirect_to = self.redirect_to.clone();
///         let is_authenticated = true; // Replace with actual check
///
///         Box::pin(async move {
///             if is_authenticated {
///                 GuardResult::allow()
///             } else {
///                 GuardResult::redirect(redirect_to)
///             }
///         })
///     }
/// }
/// ```
///
/// # For Simple Guards
///
/// Use the `guard_fn` helper to create guards from async closures:
///
/// ```no_run
/// use gpui_navigator::{guard_fn, GuardResult};
///
/// let guard = guard_fn(|_cx, _request| async move {
///     // Replace with actual authentication check
///     let is_authenticated = true;
///     if is_authenticated {
///         GuardResult::allow()
///     } else {
///         GuardResult::redirect("/login")
///     }
/// });
/// ```
pub trait RouteGuard: Send + Sync + 'static {
    /// The future returned by check
    type Future: Future<Output = GuardResult> + Send + 'static;

    /// Check if navigation should be allowed
    ///
    /// # Parameters
    /// - `cx`: Application context
    /// - `request`: Navigation request with extensions for context
    ///
    /// # Returns
    /// A future that resolves to:
    /// - `GuardResult::Allow` to allow navigation
    /// - `GuardResult::Deny` to block navigation
    /// - `GuardResult::Redirect` to redirect to a different path
    ///
    /// # Note
    ///
    /// Guards can access route parameters and request data through the
    /// `request` parameter to make authorization decisions.
    fn check(&self, cx: &App, request: &NavigationRequest) -> Self::Future;

    /// Get guard name (for debugging and error messages)
    fn name(&self) -> &str {
        "RouteGuard"
    }

    /// Optional priority for guard execution order
    ///
    /// Higher priority guards run first. Default is 0.
    fn priority(&self) -> i32 {
        0
    }
}

/// Boxed route guard for dynamic dispatch
pub type BoxedGuard =
    Box<dyn RouteGuard<Future = Pin<Box<dyn Future<Output = GuardResult> + Send>>>>;

/// Create a guard from an async function or closure
///
/// This is a convenience helper for creating guards without manually implementing
/// the `RouteGuard` trait.
///
/// # Example
///
/// ```no_run
/// use gpui_navigator::{guard_fn, GuardResult};
///
/// let auth_guard = guard_fn(|_cx, _request| async move {
///     // Replace with actual authentication check
///     let is_authenticated = true;
///     if is_authenticated {
///         GuardResult::allow()
///     } else {
///         GuardResult::redirect("/login")
///     }
/// });
/// ```
pub fn guard_fn<F, Fut>(f: F) -> FnGuard<F>
where
    F: Fn(&App, &NavigationRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = GuardResult> + Send + 'static,
{
    FnGuard { f }
}

/// Guard created from a function or closure
pub struct FnGuard<F> {
    f: F,
}

impl<F, Fut> RouteGuard for FnGuard<F>
where
    F: Fn(&App, &NavigationRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = GuardResult> + Send + 'static,
{
    type Future = Fut;

    fn check(&self, cx: &App, request: &NavigationRequest) -> Self::Future {
        (self.f)(cx, request)
    }
}

/// Guard context provides information about the navigation
#[derive(Debug, Clone)]
pub struct GuardContext {
    /// Source route
    pub from: Option<String>,

    /// Target route
    pub to: String,

    /// Target route match
    pub to_match: RouteMatch,
}

impl GuardContext {
    /// Create a new guard context
    pub fn new(from: Option<String>, to: String, to_match: RouteMatch) -> Self {
        Self { from, to, to_match }
    }

    /// Get parameter from target route
    pub fn param(&self, key: &str) -> Option<&String> {
        self.to_match.params.get(key)
    }

    /// Get query parameter from target route
    pub fn query(&self, key: &str) -> Option<&String> {
        self.to_match.query.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    #[test]
    fn test_guard_result_allow() {
        let result = GuardResult::allow();
        assert!(result.is_allow());
        assert!(!result.is_deny());
        assert!(!result.is_redirect());
        assert_eq!(result.redirect_path(), None);
    }

    #[test]
    fn test_guard_result_deny() {
        let result = GuardResult::deny("Not authorized");
        assert!(!result.is_allow());
        assert!(result.is_deny());
        assert!(!result.is_redirect());

        match result {
            GuardResult::Deny { reason } => {
                assert_eq!(reason, "Not authorized");
            }
            _ => panic!("Expected Deny"),
        }
    }

    #[test]
    fn test_guard_result_redirect() {
        let result = GuardResult::redirect("/login");
        assert!(!result.is_allow());
        assert!(!result.is_deny());
        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/login"));
    }

    #[test]
    fn test_guard_result_redirect_with_reason() {
        let result = GuardResult::redirect_with_reason("/login", "Authentication required");

        match result {
            GuardResult::Redirect { to, reason } => {
                assert_eq!(to, "/login");
                assert_eq!(reason, Some("Authentication required".to_string()));
            }
            _ => panic!("Expected Redirect"),
        }
    }

    #[test]
    fn test_guard_context() {
        let route_match = RouteMatch {
            path: "/users/123".to_string(),
            params: {
                let mut map = HashMap::new();
                map.insert("id".to_string(), "123".to_string());
                map
            },
            query: {
                let mut map = HashMap::new();
                map.insert("page".to_string(), "1".to_string());
                map
            },
        };

        let ctx = GuardContext::new(Some("/".to_string()), "/users/123".to_string(), route_match);

        assert_eq!(ctx.from, Some("/".to_string()));
        assert_eq!(ctx.to, "/users/123");
        assert_eq!(ctx.param("id"), Some(&"123".to_string()));
        assert_eq!(ctx.query("page"), Some(&"1".to_string()));
        assert_eq!(ctx.param("missing"), None);
    }

    // Mock guard for testing
    struct AlwaysAllowGuard;

    impl RouteGuard for AlwaysAllowGuard {
        type Future = Pin<Box<dyn Future<Output = GuardResult> + Send>>;

        fn check(&self, _cx: &App, _request: &NavigationRequest) -> Self::Future {
            Box::pin(async { GuardResult::allow() })
        }

        fn name(&self) -> &str {
            "AlwaysAllowGuard"
        }
    }

    #[test]
    fn test_guard_trait_name() {
        let guard = AlwaysAllowGuard;
        assert_eq!(guard.name(), "AlwaysAllowGuard");
    }

    #[test]
    fn test_guard_trait_priority() {
        let guard = AlwaysAllowGuard;
        assert_eq!(guard.priority(), 0); // Default priority
    }

    #[test]
    fn test_guard_fn_helper() {
        let guard = guard_fn(|_cx, _request| async { GuardResult::allow() });

        assert_eq!(guard.name(), "RouteGuard"); // Default name
    }
}

// ============================================================================
// Authentication and Authorization Guards
// ============================================================================

/// Type alias for authentication check function.
///
/// The function receives the application context and returns whether the user is authenticated.
pub type AuthCheckFn = Box<dyn Fn(&App) -> bool + Send + Sync>;

/// Authentication guard that checks if user is logged in.
///
/// Unlike placeholder guards, this guard is fully configurable with a custom
/// authentication check function that you provide.
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::*;
/// use gpui::App;
///
/// // Define your authentication check
/// fn is_authenticated(cx: &App) -> bool {
///     // Check your auth state here
///     cx.try_global::<AuthState>()
///         .map(|state| state.is_logged_in())
///         .unwrap_or(false)
/// }
///
/// // Use the guard
/// Route::new("/dashboard", dashboard_page)
///     .guard(AuthGuard::new(is_authenticated, "/login"))
/// # ;
/// # fn dashboard_page(_: &mut App, _: &gpui_navigator::RouteParams) -> gpui::AnyElement { todo!() }
/// # struct AuthState;
/// # impl AuthState { fn is_logged_in(&self) -> bool { false } }
/// ```
pub struct AuthGuard {
    /// Function to check if user is authenticated
    check_fn: AuthCheckFn,
    /// Path to redirect to if not authenticated
    redirect_path: String,
}

impl AuthGuard {
    /// Create a new auth guard with a custom check function and redirect path.
    ///
    /// # Arguments
    ///
    /// * `check_fn` - Function that returns `true` if the user is authenticated
    /// * `redirect_path` - Path to redirect to if authentication fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::*;
    ///
    /// let guard = AuthGuard::new(
    ///     |cx| cx.try_global::<IsLoggedIn>().is_some(),
    ///     "/login"
    /// );
    /// # struct IsLoggedIn;
    /// ```
    pub fn new<F>(check_fn: F, redirect_path: impl Into<String>) -> Self
    where
        F: Fn(&App) -> bool + Send + Sync + 'static,
    {
        Self {
            check_fn: Box::new(check_fn),
            redirect_path: redirect_path.into(),
        }
    }

    /// Create an auth guard that always allows access (for testing/development).
    ///
    /// **Warning**: Do not use in production!
    #[cfg(debug_assertions)]
    pub fn allow_all() -> Self {
        Self::new(|_| true, "/login")
    }

    /// Create an auth guard that always denies access (for testing/development).
    ///
    /// **Warning**: Do not use in production!
    #[cfg(debug_assertions)]
    pub fn deny_all(redirect_path: impl Into<String>) -> Self {
        Self::new(|_| false, redirect_path)
    }
}

impl RouteGuard for AuthGuard {
    type Future = Pin<Box<dyn Future<Output = GuardResult> + Send>>;

    fn check(&self, cx: &App, _request: &NavigationRequest) -> Self::Future {
        // Check authentication synchronously and return ready future
        let is_authenticated = (self.check_fn)(cx);
        let result = if is_authenticated {
            GuardResult::allow()
        } else {
            GuardResult::redirect_with_reason(&self.redirect_path, "Authentication required")
        };

        Box::pin(async move { result })
    }

    fn name(&self) -> &str {
        "AuthGuard"
    }

    fn priority(&self) -> i32 {
        100 // High priority - check auth first
    }
}

/// Type alias for role extraction function.
///
/// The function receives the application context and returns the user's current role(s).
pub type RoleExtractorFn = Box<dyn Fn(&App) -> Option<String> + Send + Sync>;

/// Role-based authorization guard.
///
/// Checks if user has required role for accessing a route. You must provide
/// a function that extracts the user's role from your application state.
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::*;
/// use gpui::App;
///
/// // Define how to get user's role
/// fn get_user_role(cx: &App) -> Option<String> {
///     cx.try_global::<CurrentUser>()
///         .map(|user| user.role.clone())
/// }
///
/// // Use the guard
/// Route::new("/admin", admin_page)
///     .guard(RoleGuard::new(get_user_role, "admin", Some("/forbidden")))
/// # ;
/// # fn admin_page(_: &mut App, _: &gpui_navigator::RouteParams) -> gpui::AnyElement { todo!() }
/// # struct CurrentUser { role: String }
/// ```
pub struct RoleGuard {
    /// Function to extract user's current role
    role_extractor: RoleExtractorFn,
    /// Required role
    required_role: String,
    /// Path to redirect to if unauthorized
    redirect_path: Option<String>,
}

impl RoleGuard {
    /// Create a new role guard with a role extractor function.
    ///
    /// # Arguments
    ///
    /// * `role_extractor` - Function that returns the user's current role (if any)
    /// * `required_role` - The role required to access the route
    /// * `redirect_path` - Optional path to redirect to if authorization fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::*;
    ///
    /// let guard = RoleGuard::new(
    ///     |cx| cx.try_global::<UserRole>().map(|r| r.0.clone()),
    ///     "admin",
    ///     Some("/forbidden")
    /// );
    /// # struct UserRole(String);
    /// ```
    pub fn new<F>(
        role_extractor: F,
        required_role: impl Into<String>,
        redirect_path: Option<impl Into<String>>,
    ) -> Self
    where
        F: Fn(&App) -> Option<String> + Send + Sync + 'static,
    {
        Self {
            role_extractor: Box::new(role_extractor),
            required_role: required_role.into(),
            redirect_path: redirect_path.map(Into::into),
        }
    }

    /// Check if the extracted role matches the required role
    fn has_required_role(&self, cx: &App) -> bool {
        (self.role_extractor)(cx)
            .map(|role| role == self.required_role)
            .unwrap_or(false)
    }
}

impl RouteGuard for RoleGuard {
    type Future = Pin<Box<dyn Future<Output = GuardResult> + Send>>;

    fn check(&self, cx: &App, _request: &NavigationRequest) -> Self::Future {
        let result = if self.has_required_role(cx) {
            GuardResult::allow()
        } else if let Some(redirect) = &self.redirect_path {
            GuardResult::redirect_with_reason(
                redirect,
                format!("Requires '{}' role", self.required_role),
            )
        } else {
            GuardResult::deny(format!("Missing required role: {}", self.required_role))
        };

        Box::pin(async move { result })
    }

    fn name(&self) -> &str {
        "RoleGuard"
    }

    fn priority(&self) -> i32 {
        90 // Slightly lower than auth guard
    }
}

/// Type alias for permission check function.
///
/// The function receives the application context and the required permission,
/// and returns whether the user has that permission.
pub type PermissionCheckFn = Box<dyn Fn(&App, &str) -> bool + Send + Sync>;

/// Permission-based authorization guard.
///
/// Checks if user has specific permission. You must provide a function
/// that checks permissions against your application's permission system.
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::*;
/// use gpui::App;
///
/// // Define permission check
/// fn has_permission(cx: &App, permission: &str) -> bool {
///     cx.try_global::<UserPermissions>()
///         .map(|perms| perms.contains(permission))
///         .unwrap_or(false)
/// }
///
/// // Use the guard
/// Route::new("/users/:id/delete", delete_user)
///     .guard(PermissionGuard::new(has_permission, "users.delete"))
/// # ;
/// # fn delete_user(_: &mut App, _: &gpui_navigator::RouteParams) -> gpui::AnyElement { todo!() }
/// # struct UserPermissions;
/// # impl UserPermissions { fn contains(&self, _: &str) -> bool { false } }
/// ```
pub struct PermissionGuard {
    /// Function to check if user has a permission
    check_fn: PermissionCheckFn,
    /// Required permission
    permission: String,
    /// Optional redirect path
    redirect_path: Option<String>,
}

impl PermissionGuard {
    /// Create a new permission guard with a check function.
    ///
    /// # Arguments
    ///
    /// * `check_fn` - Function that checks if user has the given permission
    /// * `permission` - The permission required to access the route
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::*;
    ///
    /// let guard = PermissionGuard::new(
    ///     |cx, perm| {
    ///         cx.try_global::<Permissions>()
    ///             .map(|p| p.has(perm))
    ///             .unwrap_or(false)
    ///     },
    ///     "users.delete"
    /// );
    /// # struct Permissions;
    /// # impl Permissions { fn has(&self, _: &str) -> bool { false } }
    /// ```
    pub fn new<F>(check_fn: F, permission: impl Into<String>) -> Self
    where
        F: Fn(&App, &str) -> bool + Send + Sync + 'static,
    {
        Self {
            check_fn: Box::new(check_fn),
            permission: permission.into(),
            redirect_path: None,
        }
    }

    /// Add a redirect path for when permission is denied.
    #[must_use]
    pub fn with_redirect(mut self, path: impl Into<String>) -> Self {
        self.redirect_path = Some(path.into());
        self
    }
}

impl RouteGuard for PermissionGuard {
    type Future = Pin<Box<dyn Future<Output = GuardResult> + Send>>;

    fn check(&self, cx: &App, _request: &NavigationRequest) -> Self::Future {
        let has_perm = (self.check_fn)(cx, &self.permission);
        let result = if has_perm {
            GuardResult::allow()
        } else if let Some(redirect) = &self.redirect_path {
            GuardResult::redirect_with_reason(
                redirect,
                format!("Missing permission: {}", self.permission),
            )
        } else {
            GuardResult::deny(format!("Missing permission: {}", self.permission))
        };

        Box::pin(async move { result })
    }

    fn name(&self) -> &str {
        "PermissionGuard"
    }

    fn priority(&self) -> i32 {
        80
    }
}

// ============================================================================
// Guard Composition
// ============================================================================

// ============================================================================
// Guard Composition
// ============================================================================

/// Combines multiple guards with AND logic
///
/// All guards must allow navigation for the combined guard to allow.
/// If any guard denies or redirects, that result is returned.
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::{Guards, AuthGuard, RoleGuard};
///
/// // Builder syntax for combining guards
/// let guard = Guards::builder()
///     .guard(AuthGuard::new(|_| true, "/login"))
///     .guard(RoleGuard::new(|_| Some("admin".into()), "admin", None::<&str>))
///     .build();
/// ```
pub struct Guards {
    guards: Vec<BoxedGuard>,
}

impl Guards {
    /// Create a new AND composition of guards
    ///
    /// # Example
    /// ```ignore
    /// use gpui_navigator::{Guards, BoxedGuard};
    ///
    /// let guard = Guards::new(vec![
    ///     // Add boxed guards here
    /// ]);
    /// ```
    pub fn new(guards: Vec<BoxedGuard>) -> Self {
        Self { guards }
    }

    /// Create from individual guards (auto-boxing)
    pub fn from_guards(guards: impl IntoIterator<Item = BoxedGuard>) -> Self {
        Self {
            guards: guards.into_iter().collect(),
        }
    }

    /// Start building a guard composition
    pub fn builder() -> GuardBuilder {
        GuardBuilder::new()
    }
}

/// Helper macro for creating Guards composition
///
/// # Example
/// ```ignore
/// use gpui_navigator::{guards, AuthGuard, RoleGuard};
///
/// let guard = guards![
///     AuthGuard::new(|_| true, "/login"),
/// ];
/// ```
#[macro_export]
macro_rules! guards {
($($guard:expr),* $(,)?) => {
$crate::guards::Guards::new(
    vec![$(Box::new($guard) as Box<dyn $crate::guards::RouteGuard>),*]
)
};
}

/// Builder for Guards with fluent API
pub struct GuardBuilder {
    guards: Vec<BoxedGuard>,
}

impl GuardBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self { guards: Vec::new() }
    }

    /// Add a guard to the composition
    pub fn guard<G>(mut self, guard: G) -> Self
    where
        G: RouteGuard<Future = Pin<Box<dyn Future<Output = GuardResult> + Send>>>,
    {
        self.guards.push(Box::new(guard));
        self
    }

    /// Add a boxed guard
    pub fn boxed_guard(mut self, guard: BoxedGuard) -> Self {
        self.guards.push(guard);
        self
    }

    /// Build the final Guards
    pub fn build(self) -> Guards {
        Guards::new(self.guards)
    }
}

impl Default for GuardBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RouteGuard for Guards {
    type Future = Pin<Box<dyn Future<Output = GuardResult> + Send + 'static>>;

    fn check(&self, cx: &App, request: &NavigationRequest) -> Self::Future {
        // Execute guards in priority order
        let mut sorted_guards: Vec<_> = self.guards.iter().collect();
        sorted_guards.sort_by_key(|g| -g.priority());

        let mut futures = Vec::new();
        for guard in sorted_guards {
            futures.push(guard.check(cx, request));
        }

        Box::pin(async move {
            for future in futures {
                match future.await {
                    GuardResult::Allow => continue,
                    other => return other,
                }
            }
            GuardResult::Allow
        })
    }

    fn name(&self) -> &str {
        "Guards"
    }

    fn priority(&self) -> i32 {
        // Priority is max of all child guards
        self.guards.iter().map(|g| g.priority()).max().unwrap_or(0)
    }
}

/// Inverts a guard result
///
/// Allow becomes Deny, Deny becomes Allow, Redirect is preserved.
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::{NotGuard, AuthGuard};
///
/// // Allow only if NOT authenticated (for login page)
/// let guard = NotGuard::new(AuthGuard::new(|_| true, "/login"));
/// ```
pub struct NotGuard {
    guard: BoxedGuard,
}

impl NotGuard {
    /// Create a new NOT guard
    pub fn new<G>(guard: G) -> Self
    where
        G: RouteGuard<Future = Pin<Box<dyn Future<Output = GuardResult> + Send>>>,
    {
        Self {
            guard: Box::new(guard),
        }
    }

    /// Create from a boxed guard
    pub fn from_boxed(guard: BoxedGuard) -> Self {
        Self { guard }
    }
}

impl RouteGuard for NotGuard {
    type Future = Pin<Box<dyn Future<Output = GuardResult> + Send + 'static>>;

    fn check(&self, cx: &App, request: &NavigationRequest) -> Self::Future {
        let future = self.guard.check(cx, request);

        Box::pin(async move {
            match future.await {
                GuardResult::Allow => GuardResult::deny("Inverted: guard allowed but NOT expected"),
                GuardResult::Deny { .. } => GuardResult::Allow,
                redirect @ GuardResult::Redirect { .. } => redirect, // Preserve redirect
            }
        })
    }

    fn name(&self) -> &str {
        "NotGuard"
    }

    fn priority(&self) -> i32 {
        self.guard.priority()
    }
}

// Additional imports for composition tests

#[cfg(test)]
mod auth_tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn test_auth_guard_allows_authenticated(cx: &mut TestAppContext) {
        // Create guard that always returns authenticated
        let guard = AuthGuard::new(|_| true, "/login");
        assert_eq!(guard.name(), "AuthGuard");
        assert_eq!(guard.priority(), 100);

        let request = NavigationRequest::new("/dashboard".to_string());
        let result = cx.update(|cx| pollster::block_on(guard.check(cx, &request)));

        assert!(result.is_allow());
    }

    #[gpui::test]
    fn test_auth_guard_blocks_unauthenticated(cx: &mut TestAppContext) {
        // Create guard that always returns not authenticated
        let guard = AuthGuard::new(|_| false, "/login");
        let request = NavigationRequest::new("/dashboard".to_string());

        let result = cx.update(|cx| pollster::block_on(guard.check(cx, &request)));

        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/login"));
    }

    #[gpui::test]
    fn test_role_guard_allows_correct_role(cx: &mut TestAppContext) {
        // Create guard that returns "admin" role
        let guard = RoleGuard::new(|_| Some("admin".to_string()), "admin", None::<String>);
        assert_eq!(guard.name(), "RoleGuard");
        assert_eq!(guard.priority(), 90);

        let request = NavigationRequest::new("/admin".to_string());
        let result = cx.update(|cx| pollster::block_on(guard.check(cx, &request)));

        assert!(result.is_allow());
    }

    #[gpui::test]
    fn test_role_guard_with_redirect(cx: &mut TestAppContext) {
        // Create guard that returns wrong role
        let guard = RoleGuard::new(|_| Some("user".to_string()), "admin", Some("/403"));
        let request = NavigationRequest::new("/admin".to_string());

        let result = cx.update(|cx| pollster::block_on(guard.check(cx, &request)));

        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/403"));
    }

    #[gpui::test]
    fn test_role_guard_deny_without_redirect(cx: &mut TestAppContext) {
        // Create guard that returns no role
        let guard = RoleGuard::new(|_| None, "admin", None::<String>);
        let request = NavigationRequest::new("/admin".to_string());

        let result = cx.update(|cx| pollster::block_on(guard.check(cx, &request)));

        assert!(result.is_deny());
    }

    #[gpui::test]
    fn test_permission_guard_allows(cx: &mut TestAppContext) {
        // Create guard that always allows
        let guard = PermissionGuard::new(|_, _| true, "users.delete");
        assert_eq!(guard.name(), "PermissionGuard");

        let request = NavigationRequest::new("/users/123/delete".to_string());
        let result = cx.update(|cx| pollster::block_on(guard.check(cx, &request)));

        assert!(result.is_allow());
    }

    #[gpui::test]
    fn test_permission_guard_denies(cx: &mut TestAppContext) {
        // Create guard that always denies
        let guard = PermissionGuard::new(|_, _| false, "users.delete");
        let request = NavigationRequest::new("/users/123/delete".to_string());

        let result = cx.update(|cx| pollster::block_on(guard.check(cx, &request)));

        assert!(result.is_deny());
    }

    #[gpui::test]
    fn test_permission_guard_with_redirect(cx: &mut TestAppContext) {
        let guard = PermissionGuard::new(|_, _| false, "users.delete").with_redirect("/forbidden");
        let request = NavigationRequest::new("/users/123/delete".to_string());

        let result = cx.update(|cx| pollster::block_on(guard.check(cx, &request)));

        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/forbidden"));
    }
}
