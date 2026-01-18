# GPUI Application Pattern

## Correct App::new() usage:

```rust
fn main() {
    App::new().run(|cx: &mut AppContext| {
        // Initialize global state
        cx.set_global(ThemeData::default());
        
        // Initialize router
        init_router(cx, |router| {
            router.add_route(/* routes */);
        });
        
        // Create and open window - REQUIRED
        cx.open_window(WindowOptions::default(), |cx| {
            cx.new_view(|_cx| MyAppView)
        }).unwrap();
    });
}
```

## Key points:
- App::new() takes no arguments
- Must call cx.open_window() inside run closure
- Window creation is mandatory for GPUI apps
- Use WindowOptions::default() for basic window
- Return value from new_view must implement Render trait

## Common mistakes:
- ❌ App::new().run() without window creation
- ❌ Trying to pass arguments to App::new()
- ❌ Not implementing Render trait correctly
- ❌ Using ViewContext instead of AppContext in main

## Render trait signature:
```rust
impl Render for MyView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div().child("content")
    }
}
```

## Router outlet usage:
```rust
// router_outlet() takes no arguments
div().child(router_outlet())

// NOT: router_outlet(cx)
```

## GPUI styling methods:
```rust
// Use when() method - requires FluentBuilder trait in scope
use gpui::prelude::*; // Import prelude for when(), hover(), etc.

div()
    .when(condition, |this| this.bg(color))
    .hover(|this| this.bg(hover_color))
```
