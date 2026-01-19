# GPUI Navigator

[![Crates.io](https://img.shields.io/crates/v/gpui-navigator.svg)](https://crates.io/crates/gpui-navigator)
[![Documentation](https://docs.rs/gpui-navigator/badge.svg)](https://docs.rs/gpui-navigator)
[![License](https://img.shields.io/crates/l/gpui-navigator.svg)](LICENSE-MIT)
[![CI](https://github.com/vanyastaff/gpui-navigator/workflows/CI/badge.svg)](https://github.com/vanyastaff/gpui-navigator/actions)

A declarative navigation library for [GPUI](https://gpui.rs) with support for nested routes, transitions, guards, and middleware.

## Features

- 🎯 **Declarative Route Definition** - Define routes with a fluent builder API
- 🎨 **Route Transitions** - Built-in fade, slide, and scale animations
- 🔀 **Nested Routing** - Support for parent/child route hierarchies with `RouterOutlet`
- 🛡️ **Route Guards** - Authentication, authorization, and custom guards
- 🔌 **Middleware** - Before/after hooks for navigation events
- 📝 **Named Routes** - Navigate using route names instead of paths
- 🔍 **Route Matching** - Pattern matching with parameters and constraints
- 📊 **Performance** - Route cache for optimized lookups
- ⚡ **Error Handling** - Custom 404 and error handlers

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
gpui-navigator = "0.1"
gpui = "0.2"
```

## Quick Start

```rust
use gpui::*;
use gpui-navigator::*;

fn main() {
    Application::new().run(|cx| {
        // Initialize router with routes
        init_router(cx, |router| {
            router.add_route(
                Route::new("/", home_page)
                    .transition(Transition::fade(300))
            );
            
            router.add_route(
                Route::new("/about", about_page)
                    .transition(Transition::slide_left(400))
            );
        });

        // Open window
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| AppView)
        })
    });
}

fn home_page(_cx: &mut App, _params: &RouteParams) -> AnyElement {
    div().child("Home Page").into_any_element()
}

fn about_page(_cx: &mut App, _params: &RouteParams) -> AnyElement {
    div().child("About Page").into_any_element()
}

struct AppView;

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(RouterOutlet::new())
    }
}
```

## Navigation

The library provides a Flutter-style navigation API:

```rust
use gpui-navigator::Navigator;

// Push new route
Navigator::push(cx, "/profile");

// Replace current route
Navigator::replace(cx, "/login");

// Go back
Navigator::pop(cx);

// Go forward
Navigator::forward(cx);

// Get current path
let path = Navigator::current_path(cx);

// Check navigation state
if Navigator::can_pop(cx) {
    Navigator::pop(cx);
}
```

### Flutter-style API

```rust
// Navigator.of(context) style
Navigator::of(cx).push("/users").pop();

// Or direct static methods
Navigator::push(cx, "/users/123");
```

## Route Transitions

The router supports smooth transitions between routes:

```rust
use gpui-navigator::*;

// Fade transition
Route::new("/fade", page).transition(Transition::fade(300))

// Slide transitions
Route::new("/left", page).transition(Transition::slide_left(400))
Route::new("/right", page).transition(Transition::slide_right(400))
Route::new("/up", page).transition(Transition::slide_up(400))
Route::new("/down", page).transition(Transition::slide_down(400))

// Scale transitions
Route::new("/zoom-in", page).transition(Transition::zoom_in(350))
Route::new("/zoom-out", page).transition(Transition::zoom_out(350))
Route::new("/custom", page).transition(Transition::scale(0.8, 1.2, 400))

// No transition
Route::new("/instant", page).transition(Transition::None)
```

## Nested Routes

Create hierarchical route structures with `RouterOutlet`:

```rust
use gpui-navigator::*;

fn dashboard_layout(_cx: &mut App, _params: &RouteParams) -> AnyElement {
    div()
        .flex()
        .child("Dashboard Header")
        .child(RouterOutlet::new())  // Child routes render here
        .into_any_element()
}

// Configure nested routes
init_router(cx, |router| {
    router.add_route(
        Route::new("/dashboard", dashboard_layout)
            .children(vec![
                Route::new("overview", overview_page),
                Route::new("settings", settings_page),
                Route::new("analytics", analytics_page),
            ])
    );
});
```

## Route Parameters

Extract parameters from route paths:

```rust
use gpui-navigator::*;

// Define route with parameter
Route::new("/users/:id", user_profile)

// Access parameters in handler
fn user_profile(_cx: &mut App, params: &RouteParams) -> AnyElement {
    let user_id = params.get("id").unwrap_or(&"unknown".to_string());
    div()
        .child(format!("User Profile: {}", user_id))
        .into_any_element()
}

// Navigate with parameters
Navigator::push(cx, "/users/123");

// Type-safe parameter extraction
let id: Option<u32> = params.get_as("id");
```

## Route Guards

Protect routes with authentication and authorization:

```rust
use gpui-navigator::*;

// Authentication guard with custom check function
fn is_authenticated(cx: &App) -> bool {
    cx.try_global::<AuthState>()
        .map(|state| state.is_logged_in())
        .unwrap_or(false)
}

Route::new("/profile", profile_page)
    .guard(AuthGuard::new(is_authenticated, "/login"))

// Role-based guard
fn get_user_role(cx: &App) -> Option<String> {
    cx.try_global::<CurrentUser>()
        .map(|user| user.role.clone())
}

Route::new("/admin", admin_page)
    .guard(RoleGuard::new(get_user_role, "admin", Some("/forbidden")))

// Permission-based guard
fn has_permission(cx: &App, permission: &str) -> bool {
    cx.try_global::<UserPermissions>()
        .map(|perms| perms.contains(permission))
        .unwrap_or(false)
}

Route::new("/users/:id/delete", delete_user)
    .guard(PermissionGuard::new(has_permission, "users.delete"))

// Custom guard with closure
Route::new("/premium", premium_page)
    .guard(guard_fn(|cx, _request| async move {
        if is_premium_user(cx) {
            GuardResult::allow()
        } else {
            GuardResult::redirect("/upgrade")
        }
    }))
```

## Middleware

Add before/after hooks to routes:

```rust
use gpui-navigator::*;

// Create middleware from functions
struct LoggingMiddleware;

impl RouteMiddleware for LoggingMiddleware {
    type Future = Pin<Box<dyn Future<Output = ()> + Send>>;

    fn before_navigation(&self, _cx: &App, request: &NavigationRequest) -> Self::Future {
        log::info!("Navigating to: {}", request.to);
        Box::pin(async {})
    }

    fn after_navigation(&self, _cx: &App, request: &NavigationRequest) -> Self::Future {
        log::info!("Navigated to: {}", request.to);
        Box::pin(async {})
    }
}

Route::new("/", home_page)
    .middleware(LoggingMiddleware)
```

## Named Routes

Navigate using route names:

```rust
use gpui-navigator::*;

// Define named route
Route::new("/profile/:id", profile_page)
    .name("user-profile")

// Navigate by name
let mut params = RouteParams::new();
params.set("id".to_string(), "123".to_string());
Navigator::push_named(cx, "user-profile", &params);

// Generate URL from name
if let Some(url) = Navigator::url_for(cx, "user-profile", &params) {
    // url = "/profile/123"
}
```

## Error Handling

Custom error and 404 handlers:

```rust
use gpui-navigator::*;

init_router(cx, |router| {
    router
        .error_handlers(ErrorHandlers::new()
            .on_not_found(|path, _cx| {
                div().child(format!("404: {} not found", path)).into_any_element()
            })
            .on_error(|error, _cx| {
                div().child(format!("Error: {}", error)).into_any_element()
            })
        );
});
```

## Examples

Run the transition demo:

```bash
cargo run --example transition_demo
```

## Architecture

### Module Structure

| Module | Description |
|--------|-------------|
| `context` | Global router state and Navigator API |
| `route` | Route definition and configuration |
| `router` | Router state management |
| `transition` | Animation definitions |
| `guards` | Authentication/authorization guards |
| `middleware` | Navigation middleware |
| `lifecycle` | Route lifecycle hooks |
| `nested` | Nested routing and RouterOutlet |
| `params` | Route and query parameters |
| `error` | Error handling |
| `widgets` | UI components (RouterLink, RouterOutlet) |

### Route Matching

Routes are matched using patterns with support for:

- Static segments: `/about`
- Dynamic parameters: `/users/:id`
- Wildcards: `/docs/*path`
- Constraints: `/users/:id<\d+>` (numeric only)

### Performance

- Route cache for O(1) lookups after first match
- Efficient parent route resolution
- Minimal allocations during navigation
- Cache statistics for monitoring

## Minimum Supported Rust Version (MSRV)

This crate requires Rust 1.75 or later.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Acknowledgments

- Inspired by [Flutter Navigator](https://api.flutter.dev/flutter/widgets/Navigator-class.html)
- Built for [GPUI](https://gpui.rs) by Zed Industries