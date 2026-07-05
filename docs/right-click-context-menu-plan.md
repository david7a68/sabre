# Right-click context menu implementation plan

## Goals

- Add first-class immediate-mode support for context menus opened by right-clicking a widget or region.
- Reuse the existing overlay, modal dismissal, hit-testing, styling, and keyboard navigation patterns already used by dropdown menus.
- Keep the API small enough for examples to read naturally while leaving room for custom menu item contents.

## Current code touchpoints

- `Input` already tracks per-frame pointer position and left, right, and middle button counts through `MouseButtonState`.
- `Interaction::compute` currently hard-codes left-button activation; context menu triggers need equivalent right-button edge detection without duplicating layer and modal hit-testing logic.
- `UiBuilder` already exposes absolute and modal overlay children, which can position a menu at a screen-space pointer coordinate and block lower layers while open.
- `Dropdown` is the closest existing widget: it persists open state in `WidgetState::custom_data`, renders a modal overlay above base content, dismisses on outside click, supports keyboard navigation, and delegates menu row content to a trait.

## Proposed public API

1. Add a `ContextMenu` widget exported from `ui::widget`.
2. Add convenience methods to `UiBuilderWidgetsExt`, for example:
   - `context_menu(id, trigger, build_menu)` for wrapping a region or widget that should open a menu.
   - `context_menu_at(id, position, build_menu)` for advanced callers that already decide when and where to open.
3. Represent item selection similarly to `Dropdown::finish`:
   - `ContextMenu::item(label)` and `ContextMenu::item_with(callback)` append selectable rows.
   - `finish()` returns `Option<usize>`; callers map indexes to application actions.
4. Expose both API layers:
   - A low-level `ContextMenu` builder for advanced composition and custom trigger handling.
   - A high-level `context_menu` closure helper for the common wrap-a-region case.
5. Expose a compact interaction helper for custom triggers:
   - `Interaction::compute_for_button(builder, MouseButton::Right, ClickBehavior::OnPress, interest)` or a small `PointerButton` enum in the crate API.
   - Right-click activation should use the same `ClickBehavior` default as current UI clicks, so it opens on press unless the caller explicitly requests release behavior.

## State model

- Store menu open/closed state in the trigger/root widget's `WidgetState::custom_data`.
- Store the menu anchor as two `f32::to_bits` values for the pointer x/y coordinate captured on the right-button press.
- Store transient overlay navigation state separately, matching `Dropdown`'s highlighted-index and keyboard-active fields.
- Close the menu when any of these happen:
  - A menu item activates.
  - Escape is pressed while the menu owns focus.
  - A click lands on the full-screen dismiss layer outside the menu.
  - The trigger/root disappears and its state is eventually cleaned up by existing widget-state lifecycle behavior.

## Rendering and input flow

1. During the trigger region build, compute hover and right-button activation using the same layer-aware rules and default click timing as normal interactions.
2. On activation, persist `is_open = true` and `anchor = input.pointer`.
3. When open, create a full-window modal dismiss child at a layer above the trigger.
4. Create the menu panel as an absolute-position modal overlay at the stored anchor.
5. Apply a new `StyleClass::ContextMenu` to the panel and a new `StyleClass::ContextMenuItem` to rows, or alias these defaults to the dropdown menu/item styles initially.
6. Clamp or flip the menu position in layout so it stays inside the window bounds; prefer the same behavior users expect from dropdown `flip_y` when possible.
7. Give the menu focus while open so Escape, ArrowUp, ArrowDown, Enter, and optionally Home/End can be handled by only the active menu.

## Dropdown reuse assessment

A large share of the context menu implementation can be copied from, extracted out of, or directly patterned after `Dropdown`. Roughly two thirds of the widget mechanics are reusable; the main new work is trigger detection, pointer anchoring, and API shape.

### Directly reusable or extractable

- Modal overlay structure: the full-window dismiss layer, overlay panel layer, and strict layer ordering can mirror `Dropdown` almost exactly.
- Open/close persistence: the root `WidgetState::custom_data` pattern can be reused, with the context menu root state storing `is_open` plus the anchor point instead of trigger width.
- Overlay navigation state: highlighted index and keyboard-active state can be reused as-is or extracted into a shared menu navigation state.
- Item rows: item counting, hover/highlight selection, activation, close-on-select, and `DropdownItem`-style callback content are directly reusable.
- Keyboard handling: ArrowUp, ArrowDown, Enter, and Escape behavior can be shared, with optional Home/End added for both dropdowns and context menus later.
- Styling defaults: `ContextMenu` and `ContextMenuItem` can initially alias or clone dropdown menu/item styles, then diverge if the visual design needs it.

### Needs context-menu-specific changes

- Trigger rendering: `Dropdown` owns and renders its trigger button; `ContextMenu` should usually wrap arbitrary caller content or be opened by a low-level `context_menu_at` helper.
- Activation button: `Dropdown` uses left-click through `Interaction::compute`; context menus need a generalized right-button interaction path that still preserves existing left-click behavior.
- Anchor state: `Dropdown` anchors to the trigger layout and stores trigger width; context menus anchor to the pointer position captured when the right-click occurs.
- Positioning: dropdowns use `OverlayPosition` relative to a parent; context menus need absolute screen-space positioning plus clamping/flipping against the window edges.
- Width policy: dropdowns often match the trigger width; context menus should size to menu content unless the caller specifies a width.
- Dismiss edge cases: a right-click on a new valid trigger while another context menu is open should probably move/reopen the menu instead of being swallowed like a normal outside-click dismissal.

### Suggested refactor boundary

Instead of duplicating the entire dropdown implementation, introduce a small shared internal menu primitive after the right-button interaction helper exists:

1. Keep `Dropdown` and `ContextMenu` as separate public widgets.
2. Extract shared overlay/menu-row behavior into private helpers such as `MenuOverlayState`, `MenuItemState`, and `build_menu_item`.
3. Let each public widget own only its trigger and anchor policy:
   - `Dropdown`: left-click trigger button, parent-relative anchor, optional trigger-width matching.
   - `ContextMenu`: right-click arbitrary region, absolute pointer anchor, content-sized panel.

This keeps the first implementation small while avoiding two independent copies of keyboard navigation, dismiss handling, and item row logic.

## Implementation steps

1. Generalize pointer-button interaction:
   - Add a small enum for left/right/middle pointer buttons or accept `winit::event::MouseButton` internally.
   - Refactor `Interaction::compute` so it delegates to a button-aware function and preserves the current left-click behavior.
   - Add unit coverage for right-button press/release edge detection if the existing testing setup supports it.
2. Add style entries:
   - Extend `StyleClass` and default theme registration with context menu panel/item styles.
   - Start with dropdown-equivalent values to avoid adding a new visual language in the first patch.
3. Implement `ContextMenu`:
   - Use `Dropdown` as the structural template, but replace trigger button rendering with a caller-supplied trigger/region and absolute pointer anchoring.
   - Persist root and overlay state with compact `Pod` structs.
   - Support item building through a trait equivalent to `DropdownItem`.
4. Add builder ergonomics:
   - Export the widget and add common extension methods so examples do not need to manually instantiate internals.
   - Provide both the low-level builder API and the high-level closure helper.
   - Document expected call order: build trigger each frame, then add menu items, then call `finish`.
5. Add an example:
   - Create `examples/context_menu.rs` showing a surface or label that opens a menu on right-click and updates visible state based on the selected item.
6. Test and polish:
   - Run `cargo fmt`.
   - Run `cargo test`.
   - Run `cargo clippy --workspace --all-targets -- -D warnings` if the existing project is clippy-clean.
   - Manually run the example and verify opening, outside dismissal, item activation, Escape, arrow navigation, and edge-of-window placement.

## Resolved design decisions

- Right-click activation should match current UI click behavior. Since `ClickBehavior::OnPress` is the current default, a context menu opens on right-button press by default.
- Expose both API layers: a low-level menu builder for control and a high-level closure helper for the common case.
- Return index-based item selection, matching `Dropdown::finish`, instead of introducing typed action callbacks in the first version.

## Native OS integration trade-offs

Native OS context menus are worth considering, but they should not replace the in-canvas menu in the first implementation. The recommended path is to build the in-canvas widget first, keep the public API action/index-based and platform-neutral, and leave an internal abstraction seam where a future native backend could be selected for simple menus.

### Pros

- Native menus automatically match platform expectations for visuals, spacing, pointer behavior, accessibility, keyboard navigation, and high-DPI rendering.
- They may provide better integration with screen readers, OS-level services, input methods, and platform conventions than a custom-rendered menu.
- They avoid reimplementing subtle menu behavior such as nested submenu timing, disabled item semantics, separators, and platform-specific dismissal rules.

### Cons

- Native menus are harder to integrate with an immediate-mode, GPU-rendered widget tree because the menu lives outside the framework's layout, styling, z-layer, focus, and modal input systems.
- Cross-platform APIs differ substantially, so Linux, Windows, macOS, and web targets may need separate implementations or a dependency that constrains supported platforms.
- Native menus cannot easily host arbitrary framework widgets, custom drawing, theme-driven rows, or in-canvas layout features.
- Testing and examples become more complicated because behavior depends on OS event loops and window-system integration rather than only the framework's input state.
- A native-first implementation could force the public API toward the least common denominator before the framework's desired menu model is clear.

### Recommendation

Start with the in-canvas `ContextMenu` because it reuses existing overlay/modal/input infrastructure and supports custom immediate-mode content. Revisit native OS integration after the in-canvas API stabilizes; if pursued, support it as an optional backend for simple text/separator/disabled-item menus while falling back to the in-canvas renderer for custom content.
