# Agents.md

## Rust coding guidelines

- Prioritize code correctness and clarity. Speed and efficiency are secondary priorities unless otherwise specified.
- Do not write organizational comments, or comments that summarize the code. Use comments only to explain 'why' the code is written in some way if there is a reason that is tricky or non-obvious.
  - In particular, be extra cautious to avoid single-line comments unless absolutely necessary.
- Minimize the amount of code changed, but not at the expense of clarity.
- Avoid creative additions unless explicitly requested.

## Project Details

- The crate can be found in crates/plinth
- It is an immediate-mode GUI framework written in Rust using winit, wgpu, and parley for text.
- Widgets live in ui/widget module, and one common invocation is added as a shortcut to the UiBuilderWidgetsExt trait for user convenience.

## General Instructions

### Once you make a change

- Check for new compiler errors and warnings introduced by your changes, and fix
  them before considering the task complete.

### When stuck

- ask a clarifying question, propose a short plan, or open a draft PR with notes
- do not push large speculative changes without confirmation
