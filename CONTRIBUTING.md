# Contributing to Mindmap

Thanks for your interest in contributing! Here's how to get started.

## Getting Started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/<your-username>/mindmap.git
   cd mindmap
   ```
3. Create a branch for your work:
   ```bash
   git checkout -b my-feature
   ```
4. Make sure it builds:
   ```bash
   cargo build
   ```

## Building & Testing

Before submitting a PR, run these checks locally:

```bash
cargo fmt -- --check   # formatting
cargo clippy -- -D warnings   # lints
cargo test             # unit tests
cargo build --release  # full release build
```

CI runs formatting, linting, and tests on every push and PR. Release builds (Windows installer, macOS DMG) only run on version tags — run `cargo build --release` locally to verify your changes compile in release mode.

## Making Changes

- Keep changes focused — one feature or fix per PR
- Follow the existing code style
- Test your changes manually with a `.mm` file before submitting
- If you add a dependency, explain why in the PR description

## Submitting a Pull Request

1. Push your branch to your fork:
   ```bash
   git push origin my-feature
   ```
2. Open a Pull Request against `master`
3. Describe what your change does and why
4. PRs require approval before merging

## What to Work On

- Check [open issues](https://github.com/asulik1988/mindmap/issues) for bugs or feature requests
- If you have a new idea, open an issue first to discuss it before writing code

## Project Structure

```
src/
├── app.rs              # eframe App, top-level update loop
├── model/              # Arena-based node tree
├── layout/             # Reingold-Tilford layout algorithm
├── canvas/             # Viewport, renderer, drawing
├── style/              # Wobble borders, hachure fill, colors
├── interaction/        # Input handling, editing, search
├── history/            # Undo/redo
├── io/                 # FreeMind .mm read/write
├── export/             # SVG, PNG, Markdown, OPML
└── ui/                 # Panels and overlays
```

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
