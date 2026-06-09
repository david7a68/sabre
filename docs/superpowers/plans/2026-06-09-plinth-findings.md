# Plinth Findings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the reported P1/P2 correctness issues in `crates/plinth` while keeping the GUI framework API small and consistent.

**Architecture:** Treat this as five cohesive fixes: shell/window lifecycle, graphics frame resource lifetime, input interaction edges, layout constraint clamping, and public widget API cleanup. Prefer narrow changes in the existing modules over new abstractions, except for helpers that remove duplicated lifecycle or constraint logic.

**Tech Stack:** Rust, winit, wgpu, parley/swash text rendering, existing `crates/plinth` unit tests, and `cargo check --workspace --all-targets`.

---

## Project Style

- Keep Rust edits minimal and local to the affected module.
- Do not add organizational comments or comments that summarize code.
- Add comments only when a non-obvious reason would otherwise be hard to recover from the code.
- Prefer focused regression tests for pure UI/layout/graphics state behavior; use `cargo check --workspace --all-targets` for shell and GPU integration paths that are not practical to unit test in this crate.
- Commit steps in this file are handoff suggestions, not required execution steps for this Codex session.

---

## File Structure

- Modify `crates/plinth/src/shell/winit.rs`: centralize winit window creation and `WinitWindow` initialization; apply `WindowConfig`; register a surface for every created window; initialize `Input::window_size`.
- Modify `crates/plinth/src/graphics/context.rs`: make render tolerant of transient per-surface errors and keep multi-window rendering behavior explicit.
- Modify `crates/plinth/src/shell/app_context.rs`: replace `unwrap()` on render with non-panicking error handling and redraw scheduling.
- Modify `crates/plinth/src/graphics/surface.rs`: turn `Surface::Lost` into a recoverable `RenderError` path and reconfigure before retrying.
- Modify `crates/plinth/src/graphics/draw.rs`: retain sampled `Texture` handles for queued draw commands and remove the seeded zero-vertex draw command.
- Modify `crates/plinth/src/graphics/texture.rs`: add a `TextureFormat` routing helper for formatted texture managers.
- Modify `crates/plinth/src/graphics/glyph_cache.rs`: preserve color glyphs by drawing color glyph textures as color samples, not alpha masks; remove unused vertical subpixel variant if not implementing vertical variants.
- Modify `crates/plinth/src/graphics/shader.wgsl`: ensure sampled color glyphs use the color texture contribution correctly.
- Modify `crates/plinth/src/shell/input.rs`: add per-frame mouse press/release edge state or captured pressed widget identity.
- Modify `crates/plinth/src/ui/widget.rs`: compute `OnPress` from input-level press edges, not widget-local `was_active`.
- Modify `crates/plinth/src/ui/widget/button.rs`: stop deriving a button widget ID from the visible label alone.
- Modify `crates/plinth/src/ui/common_widgets.rs`: update shortcut methods to use the unified public `Container` trait and stable child IDs.
- Modify `crates/plinth/src/ui/container.rs` and `crates/plinth/src/ui/mod.rs`: collapse the duplicate `Container` traits into one exported trait.
- Modify `crates/plinth/src/ui/layout/compute.rs`: centralize size clamping for `Fixed`, `Fit`, `Grow`, and `Flex`.
- Test in existing inline test modules under `crates/plinth/src/ui/widget.rs`, `crates/plinth/src/ui/layout/tree.rs`, `crates/plinth/src/ui/style/registry.rs`, `crates/plinth/src/ui/style/stateful_property.rs`, and targeted graphics modules where pure unit coverage is practical.

---

### Task 1: Centralize Window Creation and Surface Registration

**Files:**
- Modify: `crates/plinth/src/shell/winit.rs`
- Modify: `crates/plinth/src/shell/input.rs` only if a helper constructor is useful
- Verify: `crates/plinth/src/graphics/context.rs`

- [ ] **Step 1: Add a focused helper for configured winit attributes**

In `crates/plinth/src/shell/winit.rs`, extract the repeated `WindowAttributes` setup into a helper near `impl<App> WinitApp<App>`:

```rust
fn window_attributes(config: &WindowConfig) -> WindowAttributes {
    WindowAttributes::default()
        .with_title(config.title.clone())
        .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height))
        .with_visible(false)
        .with_platform_attributes(Box::new(
            WindowAttributesWindows::default().with_no_redirection_bitmap(true),
        ))
}
```

- [ ] **Step 2: Add a helper to create initial input from actual window size**

Use the actual inner size after creation so `UiContext::begin_frame` never starts from a zero-sized root:

```rust
fn input_for_window(window: &dyn Window) -> Input {
    let size = window.inner_size();
    Input {
        window_size: crate::shell::input::WindowSize {
            width: size.width as f32,
            height: size.height as f32,
        },
        ..Default::default()
    }
}
```

- [ ] **Step 3: Register a graphics surface for every new window**

In `handle_deferred_commands`, replace the inline creation block with this sequence:

```rust
let window = Arc::<dyn Window>::from(
    event_loop.create_window(window_attributes(&config)).unwrap(),
);

let graphics = match self.runtime.graphics.as_mut() {
    Some(graphics) => {
        graphics.init_surface(window.clone());
        graphics
    }
    None => self
        .runtime
        .graphics
        .get_or_insert_with(|| GraphicsContext::new(window.clone())),
};
```

Keep `GraphicsContext::new(window.clone())` as the first-window path because it already creates the first `Surface`. Call `init_surface` only for later windows.

- [ ] **Step 4: Store initialized input and request first repaint**

When inserting `WinitWindow`, replace `input: Input::default()` with:

```rust
input: input_for_window(window.as_ref()),
```

After insertion, call:

```rust
window.request_redraw();
```

The existing `can_create_surfaces` repaint path may still make the first frame visible, but the explicit redraw keeps deferred windows created after startup from waiting for another event.

- [ ] **Step 5: Verify multi-window lifecycle compiles**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: PASS. No new compiler errors or warnings from the changed window/input code.

- [ ] **Step 6: Commit**

```powershell
git add crates/plinth/src/shell/winit.rs crates/plinth/src/shell/input.rs
git commit -m "fix: initialize configured windows and surfaces"
```

---

### Task 2: Make Transient Surface States Non-Panicking

**Files:**
- Modify: `crates/plinth/src/graphics/surface.rs`
- Modify: `crates/plinth/src/graphics/context.rs`
- Modify: `crates/plinth/src/shell/app_context.rs`

- [ ] **Step 1: Expand `RenderError` for recoverable surface loss**

In `crates/plinth/src/graphics/surface.rs`, add a recoverable variant:

```rust
pub enum RenderError {
    TimedOut,
    Occluded,
    SurfaceLost,
    Unknown,
}
```

- [ ] **Step 2: Replace the `unimplemented!` path**

In `Surface::next_frame`, replace the `Lost` arm with:

```rust
wgpu::CurrentSurfaceTexture::Lost => {
    self.resize_if_necessary(device);
    output = self.handle.get_current_texture();
}
```

If this still fails after the retry loop, return `RenderError::SurfaceLost` rather than panicking. Keep `Occluded` and `TimedOut` recoverable.

- [ ] **Step 3: Handle render errors in `AppContext::repaint`**

Replace:

```rust
graphics.render(outputs).unwrap();
```

with a match:

```rust
match graphics.render(outputs) {
    Ok(()) => {}
    Err(RenderError::Occluded | RenderError::TimedOut | RenderError::SurfaceLost) => {
        tracing::debug!("Skipping repaint for transient surface state");
    }
    Err(RenderError::Unknown) => {
        tracing::warn!("Skipping repaint after unknown render error");
    }
}
```

Import `RenderError` from `crate::graphics::surface`.

- [ ] **Step 4: Keep `GraphicsContext::render` frame cleanup correct**

Audit `GraphicsContext::render`: if an early `?` can skip `self.textures.end_frame()`, refactor the per-window loop to store the error and call `end_frame()` before returning. The expected shape is:

```rust
let render_result: Result<(), RenderError> = (|| {
    for (window_id, canvas) in targets {
        let canvas = canvas.storage();

        let Some(window) = self.windows.iter_mut().find(|w| w.window_id() == window_id) else {
            warn!("Window not found, skipping render.");
            continue;
        };

        window.resize_if_necessary(&self.device);

        let (target, command_buffer) =
            write_commands(&self.device, &self.queue, &self.textures, window, canvas)?;

        command_buffers.push(command_buffer);
        presents.push((window_id, target));
    }

    tracing::info_span!("submit").in_scope(|| {
        self.queue.submit(command_buffers);
    });

    tracing::info_span!("present").in_scope(|| {
        for (window_id, target) in presents {
            let Some(window) = self.windows.iter_mut().find(|w| w.window_id() == window_id) else {
                warn!("Window not found, skipping render.");
                continue;
            };

            window.pre_present_notify();
            target.present();
        }
    });

    Ok(())
})();

self.textures.end_frame();
render_result
```

- [ ] **Step 5: Verify**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: PASS. Render errors are handled without `unwrap()` or `unimplemented!`.

- [ ] **Step 6: Commit**

```powershell
git add crates/plinth/src/graphics/surface.rs crates/plinth/src/graphics/context.rs crates/plinth/src/shell/app_context.rs
git commit -m "fix: handle transient surface render states"
```

---

### Task 3: Retain Sampled Texture Handles Until Render Consumes the Frame

**Files:**
- Modify: `crates/plinth/src/graphics/draw.rs`
- Modify: `crates/plinth/src/graphics/context.rs`

- [ ] **Step 1: Add frame texture retention to `CanvasStorage`**

Extend `CanvasStorage` with a retained texture vector:

```rust
retained_textures: Vec<Texture>,
```

Because `CanvasStorage` already derives `Default`, the new `Vec<Texture>` field will default to empty. Clear it in `reset`:

```rust
self.retained_textures.clear();
```

- [ ] **Step 2: Retain sampled handles in `CanvasStorage::push`**

After resolving `color_texture` and `alpha_texture`, clone and store them before writing the draw command:

```rust
self.retained_textures.push(color_texture.clone());
self.retained_textures.push(alpha_texture.clone());
```

This keeps atlas storage slots live until the next canvas reset.

- [ ] **Step 3: Remove the seeded zero-vertex command**

In `CanvasStorage::reset`, remove the initial `DrawCommand::Draw { num_vertices: 0, ... }` push.

In `CanvasStorage::push`, replace `self.commands.last_mut().unwrap()` with:

```rust
if let Some(DrawCommand::Draw {
    color_storage_id: prev_color_texture_id,
    alpha_storage_id: prev_alpha_texture_id,
    num_vertices,
}) = self.commands.last_mut()
    && color_texture.storage_id() == *prev_color_texture_id
    && alpha_texture.storage_id() == *prev_alpha_texture_id
{
    *num_vertices += VERTICES_PER_PRIMITIVE;
} else {
    self.commands.push(DrawCommand::Draw {
        color_storage_id: color_texture.storage_id(),
        alpha_storage_id: alpha_texture.storage_id(),
        num_vertices: VERTICES_PER_PRIMITIVE,
    });
}
```

- [ ] **Step 4: Add a draw command unit test**

In `crates/plinth/src/graphics/draw.rs`, add an inline test that exercises reset without primitives:

```rust
#[test]
fn reset_does_not_seed_empty_draw_command() {
    let mut storage = CanvasStorage::default();
    storage.reset(Color::BLACK, StorageId::default(), StorageId::default());
    assert!(storage.commands().is_empty());
}
```

Import `StorageId` inside the test module from `crate::graphics::texture`.

- [ ] **Step 5: Verify**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo test -p plinth reset_does_not_seed_empty_draw_command
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: both PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates/plinth/src/graphics/draw.rs crates/plinth/src/graphics/context.rs
git commit -m "fix: retain sampled textures through rendering"
```

---

### Task 4: Use Input-Level Press Edges for `OnPress`

**Files:**
- Modify: `crates/plinth/src/shell/input.rs`
- Modify: `crates/plinth/src/shell/winit.rs`
- Modify: `crates/plinth/src/shell/app_context.rs`
- Modify: `crates/plinth/src/ui/widget.rs`

- [ ] **Step 1: Add edge fields to `Input`**

In `Input`, add:

```rust
pub left_pressed_this_frame: bool,
pub left_released_this_frame: bool,
```

Add a helper:

```rust
impl Input {
    pub(crate) fn end_frame(&mut self) {
        self.prev_pointer = self.pointer;
        self.keyboard_events.clear();
        self.left_pressed_this_frame = false;
        self.left_released_this_frame = false;
    }
}
```

- [ ] **Step 2: Set edge fields from winit pointer button events**

In `WindowEvent::PointerButton`, when handling the left mouse button:

```rust
match state {
    winit::event::ElementState::Pressed => {
        window.input.left_pressed_this_frame = true;
        window.input.mouse_state.left_click_count = click_count;
    }
    winit::event::ElementState::Released => {
        window.input.left_released_this_frame = true;
        window.input.mouse_state.left_click_count = 0;
    }
}
```

Keep right and middle button counts consistent with their pressed/released state rather than leaving a nonzero count on release.

- [ ] **Step 3: End each input frame centrally**

In `AppContext::repaint`, replace:

```rust
input.prev_pointer = input.pointer;
window.input = input;
window.input.keyboard_events.clear();
```

with:

```rust
input.end_frame();
window.input = input;
```

- [ ] **Step 4: Compute `OnPress` from the input edge**

In `Interaction::compute`, replace:

```rust
let just_pressed = is_left_down && !was_active;
let just_released = !is_left_down && was_active;
```

with:

```rust
let just_pressed = builder.input.left_pressed_this_frame;
let just_released = builder.input.left_released_this_frame && was_active;
```

Keep `OnRelease` gated by `was_active` so release activation still requires the widget to have been pressed previously.

- [ ] **Step 5: Add an interaction regression test**

In `crates/plinth/src/ui/widget.rs`, add a focused test for the pure edge logic by extracting the activation calculation into a small private function:

```rust
fn is_activated(
    behavior: ClickBehavior,
    is_hovered: bool,
    was_active: bool,
    left_pressed_this_frame: bool,
    left_released_this_frame: bool,
) -> bool {
    match behavior {
        ClickBehavior::OnPress => is_hovered && left_pressed_this_frame,
        ClickBehavior::OnRelease => is_hovered && left_released_this_frame && was_active,
    }
}
```

Then test the reported regression:

```rust
#[test]
fn on_press_does_not_activate_when_dragging_into_widget() {
    assert!(!compute_activation(ClickBehavior::OnPress, true, false, false, false));
}
```

- [ ] **Step 6: Verify**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo test -p plinth on_press_does_not_activate_when_dragging_into_widget
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: both PASS.

- [ ] **Step 7: Commit**

```powershell
git add crates/plinth/src/shell/input.rs crates/plinth/src/shell/winit.rs crates/plinth/src/shell/app_context.rs crates/plinth/src/ui/widget.rs
git commit -m "fix: base press interactions on input edges"
```

---

### Task 5: Centralize Layout Constraint Clamping

**Files:**
- Modify: `crates/plinth/src/ui/layout/compute.rs`
- Test: `crates/plinth/src/ui/layout/tree.rs`

- [ ] **Step 1: Add a local clamp helper**

In `compute.rs`, add:

```rust
fn clamp_size(size: f32, spec: Size) -> f32 {
    match spec {
        Size::Fixed(value) => value.max(0.0),
        Size::Fit { min, max } | Size::Flex { min, max } => size.clamp(min, max),
        Size::Grow => size.max(0.0),
    }
}
```

- [ ] **Step 2: Clamp major-axis flex distribution**

In `compute_major_axis_grow_sizes`, when handling `Flex { min, max }`, clamp tentative sizes to both min and max:

```rust
let actual_size = tentative_size.clamp(min, max);
let is_done = actual_size == min || actual_size == max;
```

Do not allow negative `Grow`; only grow when `remaining_size > 0.0`, and set any grow result with `.max(0.0)`.

- [ ] **Step 3: Clamp minor-axis grow**

In `compute_minor_axis_grow_sizes`, replace:

```rust
D::set_minor_size(child, remaining_size);
```

with:

```rust
D::set_minor_size(child, remaining_size.max(0.0));
```

For `Flex { min, max }` on the minor axis, apply `clamp_size(remaining_size, D::minor_size_spec(child))`.

- [ ] **Step 4: Add regression tests**

In `crates/plinth/src/ui/layout/tree.rs`, add:

```rust
#[test]
fn minor_axis_grow_does_not_become_negative() {
    let mut tree = LayoutTree::new();
    let root = tree.add(
        None,
        Atom {
            width: Fixed(100.0),
            height: Fixed(10.0),
            inner_padding: Padding {
                top: 8.0,
                bottom: 8.0,
                ..Default::default()
            },
            ..Default::default()
        },
        (),
    );
    let child = tree.add(
        Some(root),
        Atom {
            width: Fixed(10.0),
            height: Grow,
            ..Default::default()
        },
        (),
    );

    tree.compute_layout(|_, _| None);

    assert_eq!(node_result(&tree, child).height, 0.0);
}
```

Add a second test:

```rust
#[test]
fn flex_child_does_not_shrink_below_min() {
    let mut tree = LayoutTree::new();
    let root = tree.add(
        None,
        Atom {
            width: Fixed(20.0),
            height: Fixed(100.0),
            ..Default::default()
        },
        (),
    );
    let child = tree.add(
        Some(root),
        Atom {
            width: Flex {
                min: 30.0,
                max: 100.0,
            },
            height: Fixed(10.0),
            ..Default::default()
        },
        (),
    );

    tree.compute_layout(|_, _| None);

    assert_eq!(node_result(&tree, child).width, 30.0);
}
```

- [ ] **Step 5: Verify**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo test -p plinth minor_axis_grow_does_not_become_negative flex_child_does_not_shrink_below_min
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: both PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates/plinth/src/ui/layout/compute.rs crates/plinth/src/ui/layout/tree.rs
git commit -m "fix: enforce layout size constraints"
```

---

### Task 6: Unify the Public `Container` Trait

**Files:**
- Modify: `crates/plinth/src/ui/widget.rs`
- Modify: `crates/plinth/src/ui/container.rs`
- Modify: `crates/plinth/src/ui/common_widgets.rs`
- Modify: `crates/plinth/src/ui/mod.rs`

- [ ] **Step 1: Move to one public trait**

Keep `crates/plinth/src/ui/container.rs` as the source of truth and remove the duplicate `Container` trait from `crates/plinth/src/ui/widget.rs`.

- [ ] **Step 2: Update widget/common imports**

In `common_widgets.rs`, replace:

```rust
use super::widget::Container;
```

with:

```rust
use super::container::Container;
```

In `ui/mod.rs`, keep:

```rust
pub use container::Container;
```

Do not re-export a second trait from `widget`.

- [ ] **Step 3: Verify generic widget code compiles**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: PASS, and `CommonWidgetsExt<'a>: Container<'a>` resolves to the same public `plinth::ui::Container` exported by `ui/mod.rs`.

- [ ] **Step 4: Commit**

```powershell
git add crates/plinth/src/ui/widget.rs crates/plinth/src/ui/container.rs crates/plinth/src/ui/common_widgets.rs crates/plinth/src/ui/mod.rs
git commit -m "fix: expose one container trait"
```

---

### Task 7: Give Text Buttons Stable Non-Label Widget IDs

**Files:**
- Modify: `crates/plinth/src/ui/widget/button.rs`
- Modify: `crates/plinth/src/ui/common_widgets.rs`
- Test: `crates/plinth/src/ui/widget.rs` or a new inline test in `button.rs`

- [ ] **Step 1: Stop using the visible label as the child name**

Change `Button::new` so the button container always uses `builder.child()` unless the caller supplies an explicit ID through a new constructor:

```rust
pub fn new<'a>(builder: &'a mut UiBuilder<'_>, label: Option<&str>) -> Button<'a> {
    Self::with_id(builder, (), label)
}

pub fn with_id<'a>(
    builder: &'a mut UiBuilder<'_>,
    id: impl std::hash::Hash,
    label: Option<&str>,
) -> Button<'a> {
    let mut builder = builder.named_child(id);

    let (interaction, state) = Interaction::compute(
        &builder,
        ClickBehavior::OnPress,
        StateFlags::HOVERED | StateFlags::PRESSED,
    );

    builder.apply_style(StyleClass::Button, state);
    builder.set_active(state.contains(StateFlags::PRESSED));

    if let Some(label_text) = label {
        builder.text(label_text, None);
    }

    Button {
        builder,
        interaction,
    }
}
```

Using `()` with `builder.named_child(())` is acceptable only if `UiBuilder` already incorporates sibling index into child IDs. If it does not, use `builder.child()` in `new` and reserve `with_id` for explicit IDs.

- [ ] **Step 2: Add an explicit-ID shortcut**

In `CommonWidgetsExt`, keep:

```rust
fn text_button(&mut self, label: &str) -> Interaction {
    Button::new(self.builder_mut(), Some(label)).finish()
}
```

Add:

```rust
fn named_text_button(&mut self, id: impl std::hash::Hash, label: &str) -> Interaction {
    Button::with_id(self.builder_mut(), id, Some(label)).finish()
}
```

- [ ] **Step 3: Add an ID regression test**

Use a private `Button::builder_id_for_test()` accessor in `button.rs` so the test observes the actual child `WidgetId` chosen by `Button::new` before the button is finished:

```rust
#[test]
fn duplicate_button_labels_do_not_share_widget_id() {
    let mut builder = test_builder();
    let first = Button::new(&mut builder, Some("OK")).builder_id_for_test();
    let second = Button::new(&mut builder, Some("OK")).builder_id_for_test();

    assert_ne!(first, second);
}
```

Reuse an existing `UiContext::begin_frame` test setup if one exists; otherwise add the smallest local helper needed to build a root `UiBuilder`. The test must fail on the old label-derived `named_child(label_text)` behavior and pass after the change.

- [ ] **Step 4: Verify**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo test -p plinth duplicate_button_labels_do_not_share_widget_id
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: both PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/plinth/src/ui/widget/button.rs crates/plinth/src/ui/common_widgets.rs
git commit -m "fix: avoid label-derived button ids"
```

---

### Task 8: Preserve Color Glyph Rendering

**Files:**
- Modify: `crates/plinth/src/graphics/glyph_cache.rs`
- Modify: `crates/plinth/src/graphics/shader.wgsl`
- Modify: `crates/plinth/src/graphics/draw.rs` if paint command retention needs a color/alpha distinction

- [ ] **Step 1: Store whether cached glyphs are color glyphs**

Add a field to `GlyphCacheEntry`:

```rust
is_color: bool,
```

Set it from `temp_glyph.content == Content::Color`.

- [ ] **Step 2: Upload color glyphs as color textures**

In `draw_glyph_run`, keep `TextureFormat::Rgba8UnormSrgb` for `Content::Color`, but build `Paint::Sampled` differently:

```rust
let paint = if entry.is_color {
    Paint::Sampled {
        color_tint: Color::WHITE,
        color_texture: Some(entry.texture.clone()),
        alpha_texture: None,
    }
} else {
    Paint::Sampled {
        color_tint: color,
        color_texture: None,
        alpha_texture: Some(entry.texture.clone()),
    }
};
```

Then pass `paint` into the primitive.

- [ ] **Step 3: Keep shader behavior explicit**

In `shader.wgsl`, verify `Paint::Sampled` multiplies the sampled color texture by `color_tint` and uses the alpha texture only as coverage. If the current shader assumes the glyph texture is always an alpha mask, change the sampled branch so color glyphs drawn through `color_texture` preserve RGB.

- [ ] **Step 4: Remove unused vertical subpixel state**

If vertical variants are not implemented, remove `GlyphCacheKey::y_variant` and the local `y_placement` placeholder from `draw_glyph_run`. The cache key should include only actual variants:

```rust
GlyphCacheKey {
    font_id: font.data.id(),
    glyph_id,
    x_variant: x_placement.step,
    size: font_size as u16,
}
```

- [ ] **Step 5: Verify**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: PASS. If a font/rendering sample exists, run the text example and visually confirm emoji/color glyphs keep embedded color.

- [ ] **Step 6: Commit**

```powershell
git add crates/plinth/src/graphics/glyph_cache.rs crates/plinth/src/graphics/shader.wgsl crates/plinth/src/graphics/draw.rs
git commit -m "fix: preserve color glyph textures"
```

---

### Task 9: Add Texture Format Routing Helper

**Files:**
- Modify: `crates/plinth/src/graphics/texture.rs`

- [ ] **Step 1: Add the helper on `TextureManagerInner`**

In `TextureManagerInner`, add:

```rust
fn manager_for_format_mut(&mut self, format: TextureFormat) -> &mut FormattedTextureManager {
    match format {
        TextureFormat::Rgba8Unorm => &mut self.rgba_textures,
        TextureFormat::Rgba8UnormSrgb => &mut self.srgba_textures,
        TextureFormat::R8Unorm => &mut self.alpha_textures,
    }
}
```

Add an immutable version only if the file has repeated read-only matches.

- [ ] **Step 2: Replace repeated matches**

Use the helper in allocation, release, and storage lookup paths where the same `TextureFormat` match is repeated. Keep behavior unchanged.

- [ ] **Step 3: Verify**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: PASS.

- [ ] **Step 4: Commit**

```powershell
git add crates/plinth/src/graphics/texture.rs
git commit -m "refactor: centralize texture format routing"
```

---

### Task 10: Final Verification and Review

**Files:**
- Review all modified files

- [ ] **Step 1: Run targeted tests**

Run each targeted regression test added above:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo test -p plinth reset_does_not_seed_empty_draw_command
$env:CARGO_INCREMENTAL='0'; cargo test -p plinth on_press_does_not_activate_when_dragging_into_widget
$env:CARGO_INCREMENTAL='0'; cargo test -p plinth minor_axis_grow_does_not_become_negative
$env:CARGO_INCREMENTAL='0'; cargo test -p plinth flex_child_does_not_shrink_below_min
$env:CARGO_INCREMENTAL='0'; cargo test -p plinth duplicate_button_labels_do_not_share_widget_id
```

Expected: all PASS.

- [ ] **Step 2: Run full workspace check**

Run:

```powershell
$env:CARGO_INCREMENTAL='0'; cargo check --workspace --all-targets
```

Expected: PASS. This is the same verification mode reported in the findings and avoids the Windows incremental warning.

- [ ] **Step 3: Review diff for scope**

Run:

```powershell
git diff -- crates/plinth/src/shell crates/plinth/src/graphics crates/plinth/src/ui
```

Expected: only the files listed in this plan changed; no broad stylistic rewrites or unrelated refactors.

- [ ] **Step 4: Commit any remaining verification-only fixes**

```powershell
git add crates/plinth/src docs/superpowers/plans/2026-06-09-plinth-findings.md
git commit -m "test: cover plinth regression fixes"
```

Only create this commit if verification required follow-up test or compile fixes after the task commits.

---

## Notes

- The P1 items should be implemented before P2 cleanup because window/render/input failures can mask layout and widget regressions.
- The `Button::with_id` API should be kept small; do not add a larger ID system unless the existing `UiBuilder` child ID behavior cannot distinguish duplicate siblings.
- Avoid speculative visual changes. The shader/glyph task should preserve current mask glyph behavior and only route color glyphs through the sampled color texture path.
- Keep comments sparse. Where comments are already explanatory, update them only if they become inaccurate.
