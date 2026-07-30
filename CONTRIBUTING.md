# Contributing to Nexus Memory Trust

Thank you for your interest in contributing! This guide will help you get started.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Architecture](#project-architecture)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Commit Convention](#commit-convention)
- [Style Guide](#style-guide)

---

## Code of Conduct

- Be respectful and constructive
- Focus on the code, not the person
- Welcome newcomers and help them get started
- Give credit where it's due

---

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/nexus.git
   cd nexus
   ```
3. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/NexusMemoryTrust-dev-groupe/nexus.git
   ```
4. **Create a branch** for your changes:
   ```bash
   git checkout -b feature/my-feature
   ```

---

## Development Setup

### Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | 1.75+ | `rustup update` |
| Node.js | 20+ | [nodejs.org](https://nodejs.org/) |
| npm | 9+ | Ships with Node.js |
| OpenCode CLI | Latest | `npm install -g opencode` |

### Install Dependencies

```bash
# Frontend
npm install

# Verify Rust builds
cd src-tauri && cargo check && cd ..
```

### Run in Development Mode

```bash
# Terminal 1: Frontend with hot reload
npm run dev

# Terminal 2: Tauri backend with file watching
cargo tauri dev
```

---

## Project Architecture

```
nexus/
├── src-tauri/src/
│   ├── main.rs                 # Entry point, Tauri command registration
│   ├── commands/               # Tauri IPC commands (ai.rs, config.rs, memory.rs)
│   ├── core/                   # Business logic (NO storage/infra dependencies)
│   ├── storage/sqlite/         # Database layer, migrations
│   └── infra/                  # Infrastructure adapters
├── src/
│   ├── components/             # React components
│   │   ├── ai/                 # Copilot, thinking indicator
│   │   ├── layout/             # Sidebar, logo
│   │   ├── settings/           # Settings view
│   │   ├── timeline/           # Timeline view
│   │   └── graph/              # 3D knowledge graph
│   └── styles/                 # CSS
└── ai/                         # Python AI layer (optional)
```

### Key Rules

1. **Clean Architecture**: `core/` has zero dependencies on `storage/`, `infra/`, or Tauri
2. **DI through traits**: Core uses trait abstractions, not concrete types
3. **No panics in business logic**: Always return `Result<T>`
4. **Security**: AI rules compiled into binary — never expose internals

---

## Making Changes

### 1. Understand the Problem

- Read existing issues and discussions
- If no issue exists, create one describing the bug or feature
- Wait for maintainer feedback before starting large changes

### 2. Write Code

- Follow the [Style Guide](#style-guide)
- Keep changes focused — one feature or fix per PR
- Add comments for non-obvious logic
- Update documentation if your change affects the API or usage

### 3. Test Your Changes

```bash
# Rust
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test

# Frontend
cd ..
npm run lint
npm test
npm run build
```

### 4. Write Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): description

[optional body]

[optional footer]
```

**Types:**
- `feat` — New feature
- `fix` — Bug fix
- `docs` — Documentation changes
- `style` — Formatting, missing semicolons, etc. (no code change)
- `refactor` — Code restructuring without behavior change
- `perf` — Performance improvement
- `test` — Adding or updating tests
- `chore` — Build process, dependencies, CI config

**Examples:**
```
feat(copilot): add streaming thinking indicator
fix(timeline): resolve date divider alignment on Windows
docs: update installation prerequisites
refactor(graph): extract node rendering into separate module
```

---

## Testing

### Rust Tests

```bash
cd src-tauri

# Unit tests
cargo test

# With output
cargo test -- --nocapture

# Specific test
cargo test test_name
```

### Frontend Tests

```bash
# Run all tests
npm test

# Watch mode
npm test -- --watch

# Coverage
npm test -- --coverage
```

### Integration Tests

```bash
cd src-tauri
cargo test --test integration
```

### Benchmarks

```bash
cd src-tauri
cargo bench
```

---

## Pull Request Process

### Before Submitting

- [ ] Code compiles without errors
- [ ] All tests pass
- [ ] No new compiler warnings
- [ ] `cargo fmt` and `cargo clippy` pass
- [ ] TypeScript compiles (`npx tsc --noEmit`)
- [ ] Documentation updated (if applicable)

### PR Template

```markdown
## Description

[What does this PR do?]

## Changes

- [Change 1]
- [Change 2]

## Testing

[How was this tested?]

## Screenshots (if applicable)

[Add screenshots here]
```

### Review Process

1. PRs require at least one maintainer approval
2. CI must pass (Rust + Frontend)
3. Address review feedback promptly
4. Squash commits before merging (maintainer will handle)

---

## Style Guide

### Rust

- Use `rustfmt` defaults
- Prefer `thiserror` for custom error types
- Use `anyhow` for application-level errors
- Avoid `unwrap()` in production code — use `?` or `.unwrap_or()`
- Document public items with `///` doc comments

### TypeScript/React

- Use functional components with hooks
- Prefer named exports over default exports
- Use TypeScript strict mode
- Avoid `any` — use proper types
- Components in PascalCase, files matching component name

### CSS

- Use CSS custom properties for theming
- Prefer Tailwind utilities for layout
- Component-specific styles in `globals.css` with BEM-like naming

---

## Questions?

Open a [Discussion](https://github.com/NexusMemoryTrust-dev-groupe/nexus/discussions) or reach out to the maintainers.

Thank you for contributing! 🚀
