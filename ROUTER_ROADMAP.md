# Router Module Improvement Specification

## Executive Summary

This specification outlines a comprehensive redesign of the `src/router/` module to create a production-ready, Flutter/go_router-inspired routing system for GPUI desktop applications. The design focuses on **developer ergonomics**, **type safety**, **performance**, and **real-world desktop app patterns**.

### Key Goals
- **Better code organization** with clear separation of concerns
- **Enhanced API ergonomics** following Flutter/go_router conventions
- **Type safety** with compile-time guarantees where possible
- **Performance** optimized for typical desktop applications
- **Universal patterns** serving most developer needs
- **Comprehensive error handling** with 404/error pages and navigation guards
- **Deep linking and state persistence** for desktop applications
- **Nested routing** with outlet support
- **Lifecycle hooks** and navigation guards
- **Declarative route configuration**
- **Excellent documentation** and developer tooling

---

## Current State Analysis

### Strengths
- ✅ Clean Flutter-inspired API (`Navigator::push`, `Navigator::pop`)
- ✅ Basic pattern matching (static, dynamic `:param`, wildcard `*`)
- ✅ History stack with forward/back navigation
- ✅ Route caching for performance
- ✅ Good test coverage
- ✅ Global router state integration with GPUI

### Weaknesses
- ❌ No error handling (404, error pages, failed navigation)
- ❌ No navigation guards or middleware
- ❌ Limited type safety (string-based parameters)
- ❌ No nested routing or outlet support
- ❌ No route lifecycle hooks
- ❌ No state persistence/restoration
- ❌ No transition animations
- ❌ Route builder coupling to trait objects
- ❌ Sparse documentation
- ❌ No declarative route tree configuration
- ❌ Duplicate `RouteParams` definitions (params.rs vs route_builder.rs)

---

## Architecture Design

### Module Structure

```
src/router/
├── mod.rs              # Public API exports
├── core/
│   ├── mod.rs          # Core abstractions
│   ├── route.rs        # Route definition and matching
│   ├── router.rs       # Router state machine
│   ├── matcher.rs      # Advanced pattern matching (NEW)
│   └── history.rs      # Navigation history management (NEW)
├── config/
│   ├── mod.rs          # Declarative configuration
│   ├── builder.rs      # Route tree builder
│   └── validation.rs   # Compile-time validation (NEW)
├── navigation/
│   ├── mod.rs          # Navigation APIs
│   ├── navigator.rs    # Navigator trait & impl
│   ├── context.rs      # Global router context
│   └── transition.rs   # Page transitions (NEW)
├── guards/
│   ├── mod.rs          # Navigation guards
│   ├── auth.rs         # Auth guard example (NEW)
│   └── policy.rs       # Policy-based guards (NEW)
├── params/
│   ├── mod.rs          # Parameter handling
│   ├── route_params.rs # Path parameters
│   ├── query_params.rs # Query string parameters
│   └── validation.rs   # Type-safe validation (NEW)
├── error/
│   ├── mod.rs          # Error handling
│   ├── handlers.rs     # Error page handlers (NEW)
│   └── result.rs       # Navigation result types (NEW)
├── nested/
│   ├── mod.rs          # Nested routing
│   ├── outlet.rs       # Router outlet component (NEW)
│   └── resolver.rs     # Child route resolution (NEW)
├── lifecycle/
│   ├── mod.rs          # Route lifecycle
│   ├── hooks.rs        # onEnter, onExit, canDeactivate (NEW)
│   └── events.rs       # Navigation event system (NEW)
├── persistence/
│   ├── mod.rs          # State persistence
│   ├── serializer.rs   # History serialization (NEW)
│   └── storage.rs      # Storage backend trait (NEW)
├── analytics/
│   ├── mod.rs          # Analytics integration
│   └── tracker.rs      # Navigation tracking (NEW)
└── testing/
    ├── mod.rs          # Testing utilities
    └── helpers.rs      # Test mocks and builders (NEW)
```

### Type System Design

#### Core Types

```rust
// src/router/core/route.rs
pub struct Route<B> {
    config: RouteConfig,
    builder: B,
    guards: Vec<Box<dyn RouteGuard>>,
    lifecycle: RouteLifecycle,
}

// src/router/config/builder.rs
pub struct RouteConfig {
    pub path: RoutePath,
    pub name: Option<String>,
    pub children: Vec<RouteConfig>,
    pub meta: RouteMetadata,
    pub error_handler: Option<ErrorHandler>,
}

// src/router/core/matcher.rs
pub enum RoutePath {
    Static(&'static str),
    Dynamic(String),
    Pattern(RoutePattern),
}

pub struct RoutePattern {
    segments: Vec<Segment>,
    priority: u8,
}

pub enum Segment {
    Static(String),
    Param { name: String, constraint: Option<Constraint> },
    Optional(Box<Segment>),
    Wildcard,
}

// src/router/params/validation.rs
pub trait FromParam: Sized {
    type Error;
    fn from_param(s: &str) -> Result<Self, Self::Error>;
}

pub struct TypedParams<T> {
    inner: T,
    _marker: PhantomData<T>,
}

// src/router/error/result.rs
pub enum NavigationResult {
    Success(RouteMatch),
    NotFound(String),
    Blocked(BlockReason),
    Error(NavigationError),
}

pub enum BlockReason {
    GuardRejected(String),
    LifecyclePrevent,
    InvalidParams(ValidationError),
}
```

#### Generic vs Trait Object Hybrid

```rust
// Performance-critical static routes use generics
pub struct StaticRoute<F> {
    path: &'static str,
    builder: F,
}

// Dynamic routes use trait objects for flexibility
pub trait RouteBuilder: Send + Sync {
    fn build(&self, cx: &mut App, params: &RouteParams) -> AnyElement;
}

pub struct DynamicRoute {
    config: RouteConfig,
    builder: Arc<dyn RouteBuilder>,
}

// Unified interface
pub enum AnyRoute {
    Static(StaticRoute<Box<dyn Fn(&mut App) -> AnyElement + Send + Sync>>),
    Dynamic(DynamicRoute),
}
```

---

## Feature Specifications

### 1. Error Handling & 404 Pages

#### Implementation

```rust
// src/router/error/handlers.rs
pub struct ErrorHandlers {
    not_found: Box<dyn Fn(&mut App, &str) -> AnyElement + Send + Sync>,
    forbidden: Box<dyn Fn(&mut App) -> AnyElement + Send + Sync>,
    error: Box<dyn Fn(&mut App, &Error) -> AnyElement + Send + Sync>,
}

// Usage
init_router(cx, |router| {
    router
        .on_not_found(|cx, path| {
            div().child(format!("404: {} not found", path)).into_any_element()
        })
        .on_error(|cx, error| {
            div().child(format!("Error: {}", error)).into_any_element()
        })
        .routes(routes![
            Route::new("/", home_page),
            Route::new("/users/:id", user_page),
        ]);
});
```

#### Acceptance Criteria
- [ ] Global 404 handler registered with router
- [ ] Per-route error handlers override global defaults
- [ ] Error pages receive context about failed navigation
- [ ] Errors propagate correctly through nested routes
- [ ] Test coverage for all error scenarios

---

### 2. Navigation Guards & Policies

#### Guard Trait

```rust
// src/router/guards/mod.rs
#[async_trait]
pub trait RouteGuard: Send + Sync {
    async fn can_navigate(
        &self,
        cx: &App,
        from: Option<&RouteMatch>,
        to: &RouteMatch,
    ) -> GuardResult;
}

pub enum GuardResult {
    Allow,
    Deny { reason: String },
    Redirect { to: String },
}

// Example implementation
// src/router/guards/auth.rs
pub struct AuthGuard {
    required_role: String,
}

#[async_trait]
impl RouteGuard for AuthGuard {
    async fn can_navigate(&self, cx: &App, _from: Option<&RouteMatch>, _to: &RouteMatch) -> GuardResult {
        if is_authenticated(cx) && has_role(cx, &self.required_role) {
            GuardResult::Allow
        } else {
            GuardResult::Redirect { to: "/login".to_string() }
        }
    }
}

// Usage
Route::new("/admin", admin_page)
    .guard(AuthGuard { required_role: "admin".into() })
```

#### Policy-Based Guards

```rust
// src/router/guards/policy.rs
pub struct PolicyGuard<F> {
    policy: F,
}

impl<F> PolicyGuard<F>
where
    F: Fn(&App, &RouteMatch) -> bool + Send + Sync,
{
    pub fn new(policy: F) -> Self {
        Self { policy }
    }
}

// Usage
Route::new("/premium", premium_page)
    .guard(PolicyGuard::new(|cx, _route| {
        cx.global::<UserState>().is_premium
    }))
```

#### Acceptance Criteria
- [ ] Guard trait with sync and async variants
- [ ] Guards can block, allow, or redirect navigation
- [ ] Multiple guards per route (AND/OR composition)
- [ ] Guard execution order is deterministic
- [ ] Guards can access global state and route metadata
- [ ] Examples for auth, role-based, and policy guards

---

### 3. Nested Routing & Outlets

#### Outlet Component

```rust
// src/router/nested/outlet.rs
pub struct RouterOutlet {
    name: Option<String>,
}

impl RouterOutlet {
    pub fn new() -> Self {
        Self { name: None }
    }

    pub fn named(name: impl Into<String>) -> Self {
        Self { name: Some(name.into()) }
    }
}

impl Render for RouterOutlet {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let route_match = cx.global::<GlobalRouter>().current_match();

        if let Some(matched) = route_match {
            // Resolve child route for this outlet
            let child_route = resolve_outlet_route(cx, &matched, &self.name);
            child_route.build(cx)
        } else {
            div().child("No route matched")
        }
    }
}
```

#### Nested Route Configuration

```rust
// Usage
init_router(cx, |router| {
    router.routes(routes![
        Route::new("/", root_layout)
            .children(routes![
                Route::new("home", home_page),
                Route::new("about", about_page),
            ]),
        Route::new("/dashboard", dashboard_layout)
            .children(routes![
                Route::new("overview", dashboard_overview),
                Route::new("settings", dashboard_settings),
            ]),
    ]);
});

// In root_layout
fn root_layout(cx: &mut App, _params: &RouteParams) -> AnyElement {
    div()
        .child(nav_bar())
        .child(RouterOutlet::new()) // Renders child routes here
        .into_any_element()
}
```

#### Acceptance Criteria
- [ ] RouterOutlet component renders child routes
- [ ] Named outlets for complex layouts
- [ ] Parent routes can define layouts/wrappers
- [ ] Child route resolution respects path hierarchy
- [ ] Nested params accessible in all child routes
- [ ] Guards apply to parent-child chains correctly

---

### 4. Type-Safe Route Parameters

#### Runtime Validation

```rust
// src/router/params/validation.rs
pub trait ValidatedParam: Sized {
    type Error: std::error::Error;

    fn validate(s: &str) -> Result<Self, Self::Error>;
}

impl RouteParams {
    pub fn get_validated<T: ValidatedParam>(&self, key: &str) -> Result<T, ParamError> {
        let value = self.get(key).ok_or(ParamError::Missing(key.to_string()))?;
        T::validate(value).map_err(|e| ParamError::Invalid(e.to_string()))
    }
}

// Usage
fn user_page(cx: &mut App, params: &RouteParams) -> AnyElement {
    match params.get_validated::<UserId>("id") {
        Ok(user_id) => render_user(cx, user_id),
        Err(e) => error_view(e),
    }
}
```

#### Builder Pattern with Phantom Types

```rust
// src/router/params/builder.rs
pub struct RouteBuilder<State> {
    path: String,
    params: HashMap<String, String>,
    _state: PhantomData<State>,
}

pub struct NeedsId;
pub struct HasId;

impl RouteBuilder<NeedsId> {
    pub fn with_id(self, id: impl ToString) -> RouteBuilder<HasId> {
        let mut params = self.params;
        params.insert("id".to_string(), id.to_string());

        RouteBuilder {
            path: self.path,
            params,
            _state: PhantomData,
        }
    }
}

impl RouteBuilder<HasId> {
    pub fn build(self) -> RouteDescriptor {
        // Only HasId state can call build()
        RouteDescriptor {
            path: self.path,
            params: self.params
        }
    }
}

// Usage (compile-time enforcement)
let route = RouteBuilder::new("/users/:id")
    .with_id(123)  // Required!
    .build();
```

#### Acceptance Criteria
- [ ] `ValidatedParam` trait for custom types
- [ ] `get_validated()` returns `Result<T, ParamError>`
- [ ] Builder pattern enforces required params at compile time
- [ ] Common types (u32, Uuid, etc.) have ValidatedParam impls
- [ ] Clear error messages for validation failures
- [ ] Examples demonstrating both approaches

---

### 5. Route Lifecycle Hooks

#### Hook System

```rust
// src/router/lifecycle/hooks.rs
#[async_trait]
pub trait RouteLifecycle: Send + Sync {
    async fn on_enter(&self, cx: &mut App, params: &RouteParams) -> LifecycleResult {
        LifecycleResult::Continue
    }

    async fn on_exit(&self, cx: &mut App) -> LifecycleResult {
        LifecycleResult::Continue
    }

    async fn can_deactivate(&self, cx: &App) -> bool {
        true
    }
}

pub enum LifecycleResult {
    Continue,
    Abort { reason: String },
    Redirect { to: String },
}

// Usage
struct UserPageLifecycle;

#[async_trait]
impl RouteLifecycle for UserPageLifecycle {
    async fn on_enter(&self, cx: &mut App, params: &RouteParams) -> LifecycleResult {
        // Load user data
        if let Some(id) = params.get("id") {
            load_user_data(cx, id).await;
            LifecycleResult::Continue
        } else {
            LifecycleResult::Abort { reason: "Missing user ID".into() }
        }
    }

    async fn can_deactivate(&self, cx: &App) -> bool {
        // Check for unsaved changes
        !has_unsaved_changes(cx)
    }
}

Route::new("/users/:id", user_page)
    .lifecycle(UserPageLifecycle)
```

#### Event System

```rust
// src/router/lifecycle/events.rs
pub enum NavigationEvent {
    Started { from: Option<String>, to: String },
    Completed { route: RouteMatch },
    Aborted { reason: String },
    Error { error: NavigationError },
}

// Subscribe to events
router.on_navigation(|event| {
    match event {
        NavigationEvent::Started { from, to } => {
            log::info!("Navigating from {:?} to {}", from, to);
        }
        NavigationEvent::Completed { route } => {
            analytics::track_page_view(&route.path);
        }
        _ => {}
    }
});
```

#### Acceptance Criteria
- [ ] `on_enter`, `on_exit`, `can_deactivate` hooks
- [ ] Async support for data loading
- [ ] Hooks can abort or redirect navigation
- [ ] Event system for navigation lifecycle
- [ ] Multiple subscribers to navigation events
- [ ] Deterministic hook execution order

---

### 6. Declarative Route Configuration

#### Tree-Based Configuration

```rust
// src/router/config/builder.rs
#[macro_export]
macro_rules! routes {
    ($($route:expr),* $(,)?) => {
        vec![$($route),*]
    };
}

// Usage
pub fn create_routes() -> Vec<Route> {
    routes![
        Route::new("/", home_page)
            .name("home")
            .meta("title", "Home"),

        Route::new("/users", users_layout)
            .name("users")
            .children(routes![
                Route::new("", user_list)
                    .name("users.list"),
                Route::new(":id", user_detail)
                    .name("users.detail")
                    .guard(AuthGuard::new()),
                Route::new(":id/edit", user_edit)
                    .name("users.edit")
                    .guard(AuthGuard::new())
                    .lifecycle(UnsavedChangesGuard),
            ]),

        Route::new("/admin", admin_layout)
            .guard(RoleGuard::new("admin"))
            .children(routes![
                Route::new("dashboard", admin_dashboard),
                Route::new("users", admin_users),
            ]),
    ]
}

// Initialize
init_router(cx, |router| {
    router.routes(create_routes());
});
```

#### Named Routes

```rust
// Navigate by name
Navigator::push_named(cx, "users.detail", params! {
    "id" => "123"
});

// Generate URLs
let url = router.url_for("users.detail", params! {
    "id" => "123"
}); // "/users/123"
```

#### Acceptance Criteria
- [ ] `routes!` macro for clean syntax
- [ ] Method chaining for route configuration
- [ ] Named routes with `push_named()` API
- [ ] `url_for()` generates URLs from route names
- [ ] Named route lookup is O(1) with HashMap
- [ ] Circular dependency detection at initialization

---

### 7. Deep Linking & State Persistence

#### URL Synchronization

```rust
// src/router/navigation/context.rs
pub struct GlobalRouter {
    state: RouterState,
    url_handler: Option<Box<dyn UrlHandler>>,
}

pub trait UrlHandler: Send + Sync {
    fn on_url_change(&self, url: &str);
    fn get_current_url(&self) -> String;
}

// For desktop apps with custom protocol
impl UrlHandler for DesktopUrlHandler {
    fn on_url_change(&self, url: &str) {
        // Update window title, register protocol, etc.
    }

    fn get_current_url(&self) -> String {
        format!("myapp://{}", self.current_path())
    }
}
```

#### Full State Serialization

```rust
// src/router/persistence/serializer.rs
#[derive(Serialize, Deserialize)]
pub struct PersistedRouterState {
    history: Vec<String>,
    current_index: usize,
    metadata: HashMap<String, Value>,
}

impl GlobalRouter {
    pub fn serialize_state(&self) -> Result<Vec<u8>, SerializationError> {
        let state = PersistedRouterState {
            history: self.state.history.clone(),
            current_index: self.state.current,
            metadata: self.collect_metadata(),
        };

        bincode::serialize(&state)
    }

    pub fn restore_state(&mut self, data: &[u8]) -> Result<(), SerializationError> {
        let state: PersistedRouterState = bincode::deserialize(data)?;
        self.state.history = state.history;
        self.state.current = state.current_index;
        self.restore_metadata(state.metadata);
        Ok(())
    }
}

// Auto-save on navigation
router.on_navigation(|event| {
    if let NavigationEvent::Completed { .. } = event {
        save_router_state_to_disk(&router);
    }
});

// Restore on app start
fn main() {
    App::new().run(|cx| {
        init_router(cx, |router| {
            if let Ok(state) = load_router_state_from_disk() {
                router.restore_state(&state);
            }
        });
    });
}
```

#### Acceptance Criteria
- [ ] Full history serialization with bincode/serde
- [ ] Route metadata persistence (scroll position, form data)
- [ ] Restoration validates routes still exist
- [ ] Graceful fallback to home if restoration fails
- [ ] Platform-specific storage backends (files, registry, etc.)
- [ ] Opt-in per route with `.persistent()` marker

---

### 8. Programmatic Navigation with Results

#### Future-Based Navigation

```rust
// src/router/navigation/navigator.rs
pub struct NavigationHandle<T> {
    receiver: Receiver<T>,
}

impl<T> NavigationHandle<T> {
    pub async fn result(self) -> T {
        self.receiver.await.unwrap()
    }
}

impl Navigator {
    pub fn push_for_result<T: 'static>(
        cx: &mut impl BorrowAppContext,
        route: impl IntoRoute,
    ) -> NavigationHandle<T> {
        let (sender, receiver) = oneshot::channel();

        // Store sender in global state keyed by route
        cx.update_global::<GlobalRouter, _>(|router, _| {
            let descriptor = route.into_route();
            router.push_with_result(descriptor, sender);
        });

        NavigationHandle { receiver }
    }

    pub fn pop_with_result<T: 'static>(cx: &mut impl BorrowAppContext, result: T) {
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.complete_navigation(result);
            router.back();
        });
    }
}

// Usage
async fn show_dialog(cx: &mut App) {
    let handle = Navigator::push_for_result::<DialogResult>(cx, "/dialog");

    let result = handle.result().await;

    match result {
        DialogResult::Confirmed(data) => process_data(data),
        DialogResult::Cancelled => {}
    }
}

// In dialog component
fn on_confirm(cx: &mut ViewContext<Dialog>, data: Data) {
    Navigator::pop_with_result(cx, DialogResult::Confirmed(data));
}
```

#### Acceptance Criteria
- [ ] `push_for_result()` returns a handle
- [ ] `pop_with_result()` passes data to handle
- [ ] Type-safe result passing with generics
- [ ] Handle cleanup if route is dismissed without result
- [ ] Support for synchronous result callbacks
- [ ] Examples for dialogs, forms, and pickers

---

### 9. Route Transitions & Animations

#### Transition API

```rust
// src/router/navigation/transition.rs
pub enum Transition {
    None,
    Fade { duration_ms: u64 },
    Slide { direction: SlideDirection, duration_ms: u64 },
    Custom(Box<dyn TransitionAnimation>),
}

pub trait TransitionAnimation: Send + Sync {
    fn animate(
        &self,
        old_view: AnyElement,
        new_view: AnyElement,
        progress: f32,
    ) -> AnyElement;
}

// Usage
Route::new("/page", page_view)
    .transition(Transition::Fade { duration_ms: 200 })

Navigator::push(cx, "/page")
    .with_transition(Transition::Slide {
        direction: SlideDirection::Left,
        duration_ms: 300,
    });
```

#### Integration with GPUI Animations

```rust
impl RouterOutlet {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let route = cx.global::<GlobalRouter>().current_match();

        cx.with_animation(
            Animation::new(Duration::from_millis(200))
                .with_easing(cubic_bezier(0.4, 0.0, 0.2, 1.0)),
            move |cx| {
                if let Some(matched) = route {
                    matched.build(cx)
                } else {
                    div().child("Not found")
                }
            }
        )
    }
}
```

#### Acceptance Criteria
- [ ] Built-in transitions (None, Fade, Slide)
- [ ] Custom transition trait for animations
- [ ] Per-route default transitions
- [ ] Per-navigation transition overrides
- [ ] Integration with GPUI's animation system
- [ ] Smooth transitions maintain 60fps

---

### 10. Analytics Integration

#### Tracker Trait

```rust
// src/router/analytics/tracker.rs
pub trait NavigationTracker: Send + Sync {
    fn track_page_view(&self, route: &RouteMatch);
    fn track_navigation(&self, from: Option<&str>, to: &str, duration_ms: u64);
    fn track_error(&self, error: &NavigationError);
}

// Example implementation
pub struct ConsoleTracker;

impl NavigationTracker for ConsoleTracker {
    fn track_page_view(&self, route: &RouteMatch) {
        println!("[Analytics] Page view: {}", route.path);
    }

    fn track_navigation(&self, from: Option<&str>, to: &str, duration_ms: u64) {
        println!("[Analytics] Navigation {:?} -> {} took {}ms", from, to, duration_ms);
    }

    fn track_error(&self, error: &NavigationError) {
        println!("[Analytics] Navigation error: {}", error);
    }
}

// Usage
init_router(cx, |router| {
    router
        .with_tracker(ConsoleTracker)
        .routes(create_routes());
});
```

#### Acceptance Criteria
- [ ] `NavigationTracker` trait
- [ ] Automatic page view tracking
- [ ] Navigation timing measurements
- [ ] Error tracking
- [ ] Multiple trackers support (chain pattern)
- [ ] Examples for console, file, and HTTP trackers

---

### 11. Documentation & Developer Experience

#### Comprehensive Documentation

```rust
/// # Router Module
///
/// Provides declarative routing for GPUI desktop applications.
///
/// ## Quick Start
///
/// ```rust
/// use gpui_router::router::*;
///
/// fn main() {
///     App::new().run(|cx| {
///         init_router(cx, |router| {
///             router.routes(routes![
///                 Route::new("/", home_page),
///                 Route::new("/users/:id", user_page),
///             ]);
///         });
///
///         // Navigate
///         Navigator::push(cx, "/users/123");
///     });
/// }
/// ```
///
/// ## Features
///
/// - **Declarative route configuration** with nested routes
/// - **Type-safe parameters** with validation
/// - **Navigation guards** for auth and permissions
/// - **Lifecycle hooks** for data loading and cleanup
/// - **Deep linking** with state persistence
/// - **Error handling** with 404 and error pages
/// - **Transitions** with animations
///
/// ## Common Patterns
///
/// ### Nested Routes with Layouts
///
/// ```rust
/// Route::new("/dashboard", dashboard_layout)
///     .children(routes![
///         Route::new("overview", overview_page),
///         Route::new("settings", settings_page),
///     ])
/// ```
///
/// ### Authenticated Routes
///
/// ```rust
/// Route::new("/admin", admin_page)
///     .guard(AuthGuard::required())
/// ```
///
/// ### Type-Safe Parameters
///
/// ```rust
/// fn user_page(cx: &mut App, params: &RouteParams) -> AnyElement {
///     let user_id: UserId = params.get_validated("id")?;
///     // ...
/// }
/// ```
///
/// ## Architecture
///
/// The router is built on several core concepts:
///
/// - **Route**: Defines a path pattern and view builder
/// - **Navigator**: API for navigation (push, pop, replace)
/// - **Guards**: Middleware that can block navigation
/// - **Lifecycle**: Hooks for route enter/exit
/// - **Outlet**: Component that renders child routes
///
/// ## Performance
///
/// - Route matching is O(n) with early exit optimization
/// - Matched routes are cached with LRU eviction
/// - Static routes use zero-cost abstractions
/// - Typical navigation completes in < 1ms
///
/// ## Migration Guide
///
/// ### From Current Router
///
/// Old:
/// ```rust
/// init_router(cx, |router| {
///     router.add_route(Route::new("/home", home_page));
/// });
/// ```
///
/// New:
/// ```rust
/// init_router(cx, |router| {
///     router.routes(routes![
///         Route::new("/home", home_page),
///     ]);
/// });
/// ```
///
/// ### Named Routes
///
/// Old:
/// ```rust
/// Navigator::push(cx, "/users/123");
/// ```
///
/// New:
/// ```rust
/// Navigator::push_named(cx, "users.detail", params! {
///     "id" => "123"
/// });
/// ```
pub mod router;
```

#### Testing Utilities

```rust
// src/router/testing/helpers.rs
pub struct RouterTestContext {
    cx: TestAppContext,
}

impl RouterTestContext {
    pub fn new() -> Self {
        let mut cx = TestAppContext::new();
        cx.update(|cx| {
            init_router(cx, |_| {});
        });
        Self { cx }
    }

    pub fn navigate(&mut self, path: &str) {
        self.cx.update(|cx| Navigator::push(cx, path));
    }

    pub fn current_path(&self) -> String {
        self.cx.read(|cx| Navigator::current_path(cx))
    }

    pub fn assert_path(&self, expected: &str) {
        assert_eq!(self.current_path(), expected);
    }
}

// Usage in tests
#[test]
fn test_navigation() {
    let mut ctx = RouterTestContext::new();
    ctx.navigate("/users/123");
    ctx.assert_path("/users/123");
}
```

#### Error Messages

```rust
// Before
thread 'main' panicked at 'No route matched'

// After
Error: Route not found
  --> /invalid/path
  |
  | Available routes:
  |   /
  |   /users
  |   /users/:id
  |   /admin (requires auth)
  |
  | Did you mean: /users ?
```

#### Acceptance Criteria
- [ ] Module-level docs with examples
- [ ] Per-function docs with usage examples
- [ ] Migration guide from old router
- [ ] Testing utilities and mocks
- [ ] IDE autocomplete with trait bounds
- [ ] Descriptive error messages with suggestions
- [ ] Examples covering all major features

---

## Implementation Phases

### Phase 1: Core Refactoring (Week 1)
- [ ] Reorganize module structure
- [ ] Consolidate duplicate types (RouteParams)
- [ ] Implement new matcher with priority
- [ ] Extract history management
- [ ] Add comprehensive tests

### Phase 2: Error Handling & Guards (Week 2)
- [ ] Error handler system (404, errors)
- [ ] Guard trait and implementations
- [ ] Policy-based guards
- [ ] Guard composition (AND/OR)
- [ ] Examples and tests

### Phase 3: Type Safety & Validation (Week 2-3)
- [ ] ValidatedParam trait
- [ ] Builder pattern with phantom types
- [ ] Common type implementations
- [ ] Validation error handling
- [ ] Documentation and examples

### Phase 4: Nested Routing (Week 3-4)
- [ ] RouterOutlet component
- [ ] Child route resolution
- [ ] Named outlets
- [ ] Parent-child path composition
- [ ] Examples and tests

### Phase 5: Lifecycle & Events (Week 4)
- [ ] Lifecycle hook trait
- [ ] Event system
- [ ] on_enter, on_exit, can_deactivate
- [ ] Async support
- [ ] Examples and tests

### Phase 6: Advanced Features (Week 5)
- [ ] Declarative route configuration
- [ ] Named routes and url_for()
- [ ] Programmatic navigation with results
- [ ] Deep linking support
- [ ] Analytics integration

### Phase 7: Persistence & Transitions (Week 6)
- [ ] State serialization
- [ ] Persistence backends
- [ ] Transition system
- [ ] GPUI animation integration
- [ ] Examples and tests

### Phase 8: Documentation & Polish (Week 7)
- [ ] Comprehensive module docs
- [ ] Per-function documentation
- [ ] Testing utilities
- [ ] Migration guide
- [ ] Error message improvements
- [ ] Examples for all patterns

---

## Testing Strategy

### Unit Tests
- Route matching (static, dynamic, wildcards, optional segments)
- Parameter parsing and validation
- Guard execution and composition
- Lifecycle hook execution order
- Event emission and subscription
- History stack operations
- Nested route resolution

### Integration Tests
- End-to-end navigation flows
- Guard + lifecycle interactions
- Nested routes with outlets
- Error handling across routes
- State persistence and restoration
- Navigation with results
- Transition animations

### Performance Tests
- Route matching with 1000+ routes
- Cache effectiveness
- Navigation latency (< 1ms)
- Memory usage (history growth)
- Serialization/deserialization speed

### Manual Testing Checklist
- [ ] Navigate through nested routes
- [ ] Test guard blocking and redirects
- [ ] Verify lifecycle hooks fire correctly
- [ ] Test browser-like forward/back
- [ ] Persist and restore state
- [ ] Test with animations enabled
- [ ] Verify error pages render
- [ ] Test with typed parameters
- [ ] Verify analytics tracking

---

## Performance Considerations

### Route Matching Optimization
```rust
// Use trie/radix tree for large route tables
pub struct RouteTrie {
    root: Node,
}

struct Node {
    segment: Option<Segment>,
    children: HashMap<String, Node>,
    route: Option<Arc<Route>>,
}

// O(m) matching where m = path segment count
```

### Caching Strategy
```rust
pub struct RouteCache {
    cache: LruCache<String, RouteMatch>,
    max_size: usize,
}

// Cache hit: O(1)
// Cache miss: O(n) with early exit
```

### Memory Management
- Use `Arc` for shared route definitions
- `Cow<str>` for paths to avoid allocations
- LRU cache with configurable size (default: 100)
- History truncation option (max 1000 entries)

### Benchmarking Targets
- Route matching: < 100ns for cached, < 10μs for uncached
- Navigation: < 1ms total (matching + hooks + render trigger)
- Serialization: < 5ms for 1000 history entries
- Memory: < 1MB for typical app (50 routes, 100 history)

---

## Security Considerations

### Input Validation
```rust
// Prevent path traversal
fn sanitize_path(path: &str) -> String {
    path.split('/')
        .filter(|s| *s != ".." && *s != ".")
        .collect::<Vec<_>>()
        .join("/")
}

// Validate parameter names (no special chars)
fn validate_param_name(name: &str) -> Result<(), ValidationError> {
    if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(ValidationError::InvalidParamName(name.to_string()))
    }
}
```

### Safe Deserialization
```rust
impl GlobalRouter {
    pub fn restore_state(&mut self, data: &[u8]) -> Result<(), SerializationError> {
        // Validate before deserializing
        let state: PersistedRouterState = bincode::deserialize(data)?;

        // Verify all routes exist
        for path in &state.history {
            if !self.route_exists(path) {
                return Err(SerializationError::InvalidRoute(path.clone()));
            }
        }

        // Validate index bounds
        if state.current_index >= state.history.len() {
            return Err(SerializationError::InvalidIndex);
        }

        // Safe to restore
        self.state.history = state.history;
        self.state.current = state.current_index;
        Ok(())
    }
}
```

---

## Open Questions & Risks

### Questions
1. **Async in Desktop Apps**: How to handle async lifecycle hooks without blocking the main thread? Consider using `block_on` with timeout or spawn background tasks.

2. **Transition Performance**: Can we maintain 60fps with complex transitions? May need frame budget or early termination.

3. **Memory Usage**: What's acceptable memory footprint for history? Consider configurable limits and compression.

4. **Backward Compatibility**: How to migrate existing code? Provide compatibility layer or require breaking changes?

### Risks

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Performance regression | High | Benchmark before/after; optimize hot paths |
| API too complex | High | Iterative design; gather feedback |
| Breaking changes | Medium | Provide migration guide and compatibility layer |
| Async complexity | Medium | Provide sync alternatives; clear examples |
| State persistence bugs | Medium | Comprehensive testing; safe deserialization |
| Memory leaks | High | Careful Arc usage; test with large histories |

---

## Success Criteria

### Functional
- [ ] All features implemented as specified
- [ ] 90%+ test coverage
- [ ] All acceptance criteria met
- [ ] Zero regressions from current router
- [ ] Examples run successfully

### Performance
- [ ] Route matching < 10μs (99th percentile)
- [ ] Navigation latency < 1ms
- [ ] Memory usage < 2MB for typical app
- [ ] 60fps maintained during transitions

### Developer Experience
- [ ] Documentation completeness: 100%
- [ ] Example coverage: All major patterns
- [ ] Error messages: Actionable and clear
- [ ] IDE autocomplete: Works with generic types
- [ ] Migration guide: < 1 hour for typical app

### Code Quality
- [ ] No `unsafe` code (unless justified)
- [ ] Clippy warnings: Zero
- [ ] Rustfmt compliance: 100%
- [ ] Public API documented: 100%
- [ ] TODOs/FIXMEs: Zero in production code

---

## Appendix: API Examples

### Basic Navigation
```rust
// Initialize
init_router(cx, |router| {
    router.routes(routes![
        Route::new("/", home),
        Route::new("/about", about),
    ]);
});

// Navigate
Navigator::push(cx, "/about");
Navigator::pop(cx);
Navigator::replace(cx, "/home");
```

### Nested Routes
```rust
Route::new("/dashboard", dashboard_layout)
    .children(routes![
        Route::new("", dashboard_home),
        Route::new("settings", dashboard_settings),
        Route::new("profile", dashboard_profile),
    ])
```

### Route Guards
```rust
Route::new("/admin", admin_page)
    .guard(AuthGuard::required())
    .guard(RoleGuard::new("admin"))
```

### Lifecycle Hooks
```rust
Route::new("/form", form_page)
    .on_enter(|cx, params| {
        load_form_data(cx, params).await;
    })
    .on_exit(|cx| {
        save_draft(cx).await;
    })
    .can_deactivate(|cx| {
        !has_unsaved_changes(cx)
    })
```

### Type-Safe Parameters
```rust
fn user_page(cx: &mut App, params: &RouteParams) -> AnyElement {
    let user_id: UserId = params.get_validated("id")?;
    let sort_order: SortOrder = params.get_validated("sort").unwrap_or_default();

    render_user(cx, user_id, sort_order)
}
```

### Named Routes
```rust
// Define
Route::new("/users/:id", user_page)
    .name("user.detail")

// Navigate
Navigator::push_named(cx, "user.detail", params! {
    "id" => "123"
});

// Generate URL
let url = router.url_for("user.detail", params! { "id" => "123" });
```

### Error Handling
```rust
init_router(cx, |router| {
    router
        .on_not_found(|cx, path| {
            NotFoundPage::new(path).render(cx)
        })
        .on_error(|cx, error| {
            ErrorPage::new(error).render(cx)
        })
        .routes(create_routes());
});
```

### State Persistence
```rust
// Save on exit
app.on_quit(|cx| {
    let state = cx.global::<GlobalRouter>().serialize_state()?;
    save_to_disk("router_state.bin", &state)?;
});

// Restore on start
app.on_start(|cx| {
    if let Ok(state) = load_from_disk("router_state.bin") {
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.restore_state(&state)?;
        });
    }
});
```

---

## Next Steps

1. **Review this spec** with the team and stakeholders
2. **Prioritize features** if timeline is constrained
3. **Create detailed task breakdown** for Phase 1
4. **Set up benchmarking infrastructure** before starting
5. **Begin implementation** in a feature branch

---

## Handoff Instructions

**IMPORTANT: Start a NEW SESSION to implement this specification.**

When you're ready to implement, start a fresh Claude Code session and provide this spec file. This ensures:
- Clean context without interview overhead
- Focused implementation phase
- Better incremental progress tracking
- Clearer separation of planning vs execution

In the new session, say:
```
I have a detailed specification for improving the router module.
Please read ROUTER_SPEC.md and begin implementation following the phases outlined.
Start with Phase 1: Core Refactoring.
```
