//! Route lifecycle hooks

use crate::NavigationRequest;
use gpui::App;
use std::future::Future;
use std::pin::Pin;

/// Result of a lifecycle hook
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleResult {
    /// Continue with navigation
    Continue,

    /// Abort navigation with reason
    Abort { reason: String },

    /// Redirect to another path
    Redirect { to: String },
}

impl LifecycleResult {
    /// Create a continue result
    pub fn cont() -> Self {
        Self::Continue
    }

    /// Create an abort result
    pub fn abort(reason: impl Into<String>) -> Self {
        Self::Abort {
            reason: reason.into(),
        }
    }

    /// Create a redirect result
    pub fn redirect(to: impl Into<String>) -> Self {
        Self::Redirect { to: to.into() }
    }

    /// Check if lifecycle allows continuation
    pub fn allows_continue(&self) -> bool {
        matches!(self, LifecycleResult::Continue)
    }

    /// Check if lifecycle aborts
    pub fn is_abort(&self) -> bool {
        matches!(self, LifecycleResult::Abort { .. })
    }

    /// Check if lifecycle redirects
    pub fn is_redirect(&self) -> bool {
        matches!(self, LifecycleResult::Redirect { .. })
    }
}

/// Route lifecycle hooks
///
/// Lifecycle hooks allow you to run code at key points in the navigation process:
/// - `on_enter`: Called when entering a route (for data loading, setup)
/// - `on_exit`: Called when leaving a route (for cleanup, saving state)
/// - `can_deactivate`: Called to check if user can leave (for unsaved changes warning)
///
/// # Example
///
/// ```no_run
/// use gpui_router::{RouteLifecycle, LifecycleResult, NavigationRequest};
/// use std::future::Future;
/// use std::pin::Pin;
///
/// struct FormLifecycle;
///
/// impl RouteLifecycle for FormLifecycle {
///     type Future = Pin<Box<dyn Future<Output = LifecycleResult> + Send>>;
///
///     fn on_enter(&self, _cx: &gpui::App, _request: &NavigationRequest) -> Self::Future {
///         // Load form data
///         Box::pin(async { LifecycleResult::Continue })
///     }
///
///     fn on_exit(&self, _cx: &gpui::App) -> Self::Future {
///         Box::pin(async { LifecycleResult::Continue })
///     }
///
///     fn can_deactivate(&self, _cx: &gpui::App) -> Self::Future {
///         // Check for unsaved changes
///         Box::pin(async { LifecycleResult::Continue })
///     }
/// }
/// ```
pub trait RouteLifecycle: Send + Sync + 'static {
    /// The future returned by lifecycle methods
    type Future: Future<Output = LifecycleResult> + Send + 'static;

    /// Called when entering the route
    ///
    /// Use this to:
    /// - Load data for the route
    /// - Set up subscriptions
    /// - Initialize state
    /// - Validate navigation parameters
    ///
    /// Return `LifecycleResult::Abort` to prevent navigation.
    /// Return `LifecycleResult::Redirect` to navigate elsewhere.
    fn on_enter(&self, cx: &App, request: &NavigationRequest) -> Self::Future;

    /// Called when exiting the route
    ///
    /// Use this to:
    /// - Save state
    /// - Clean up subscriptions
    /// - Cancel pending operations
    ///
    /// Return `LifecycleResult::Abort` to prevent navigation.
    fn on_exit(&self, cx: &App) -> Self::Future;

    /// Check if the route can be deactivated
    ///
    /// Use this to:
    /// - Check for unsaved changes
    /// - Confirm navigation away
    /// - Validate state before leaving
    ///
    /// Return `LifecycleResult::Abort` to prevent navigation.
    fn can_deactivate(&self, cx: &App) -> Self::Future;
}

/// Type-erased lifecycle for dynamic dispatch
pub type BoxedLifecycle =
    Box<dyn RouteLifecycle<Future = Pin<Box<dyn Future<Output = LifecycleResult> + Send>>>>;

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    struct TestLifecycle {
        should_abort: bool,
        should_redirect: bool,
    }

    impl RouteLifecycle for TestLifecycle {
        type Future = Pin<Box<dyn Future<Output = LifecycleResult> + Send>>;

        fn on_enter(&self, _cx: &App, _request: &NavigationRequest) -> Self::Future {
            if self.should_abort {
                Box::pin(async { LifecycleResult::abort("Test abort") })
            } else if self.should_redirect {
                Box::pin(async { LifecycleResult::redirect("/redirect") })
            } else {
                Box::pin(async { LifecycleResult::Continue })
            }
        }

        fn on_exit(&self, _cx: &App) -> Self::Future {
            Box::pin(async { LifecycleResult::Continue })
        }

        fn can_deactivate(&self, _cx: &App) -> Self::Future {
            if self.should_abort {
                Box::pin(async { LifecycleResult::abort("Cannot leave") })
            } else {
                Box::pin(async { LifecycleResult::Continue })
            }
        }
    }

    #[gpui::test]
    fn test_lifecycle_result_continue(_cx: &mut TestAppContext) {
        let result = LifecycleResult::Continue;
        assert!(result.allows_continue());
        assert!(!result.is_abort());
        assert!(!result.is_redirect());
    }

    #[gpui::test]
    fn test_lifecycle_result_abort(_cx: &mut TestAppContext) {
        let result = LifecycleResult::abort("Test");
        assert!(!result.allows_continue());
        assert!(result.is_abort());
        assert!(!result.is_redirect());
    }

    #[gpui::test]
    fn test_lifecycle_result_redirect(_cx: &mut TestAppContext) {
        let result = LifecycleResult::redirect("/test");
        assert!(!result.allows_continue());
        assert!(!result.is_abort());
        assert!(result.is_redirect());
    }

    #[gpui::test]
    fn test_lifecycle_on_enter_continue(cx: &mut TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: false,
            should_redirect: false,
        };
        let request = NavigationRequest::new("/test".to_string());

        let result = cx.update(|cx| pollster::block_on(lifecycle.on_enter(cx, &request)));

        assert_eq!(result, LifecycleResult::Continue);
    }

    #[gpui::test]
    fn test_lifecycle_on_enter_abort(cx: &mut TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: true,
            should_redirect: false,
        };
        let request = NavigationRequest::new("/test".to_string());

        let result = cx.update(|cx| pollster::block_on(lifecycle.on_enter(cx, &request)));

        assert!(result.is_abort());
    }

    #[gpui::test]
    fn test_lifecycle_on_enter_redirect(cx: &mut TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: false,
            should_redirect: true,
        };
        let request = NavigationRequest::new("/test".to_string());

        let result = cx.update(|cx| pollster::block_on(lifecycle.on_enter(cx, &request)));

        assert!(result.is_redirect());
    }

    #[gpui::test]
    fn test_lifecycle_can_deactivate_allow(cx: &mut TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: false,
            should_redirect: false,
        };

        let result = cx.update(|cx| pollster::block_on(lifecycle.can_deactivate(cx)));

        assert_eq!(result, LifecycleResult::Continue);
    }

    #[gpui::test]
    fn test_lifecycle_can_deactivate_block(cx: &mut TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: true,
            should_redirect: false,
        };

        let result = cx.update(|cx| pollster::block_on(lifecycle.can_deactivate(cx)));

        assert!(result.is_abort());
    }
}
