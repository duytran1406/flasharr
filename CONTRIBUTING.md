# Contributing to Flasharr

Thanks for your interest in contributing to Flasharr! Here's how to get started.

## Getting Started

1. **Fork** the repository
2. **Clone** your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/flasharr.git
   cd flasharr
   ```
3. **Create a branch** for your feature or fix:
   ```bash
   git checkout -b feature/my-feature
   ```

## Development Setup

### Prerequisites

- **Rust** 1.75+ ([install](https://rustup.rs/))
- **Node.js** 20+ ([install](https://nodejs.org/))
- **Docker** (optional, for container testing)

### Running Locally

```bash
# Backend
cd backend
cargo run

# Frontend (separate terminal)
cd frontend
npm install
npm run dev
```

The frontend dev server runs on `http://localhost:5173` and proxies API calls to the backend at `http://localhost:8484`.

## Making Changes

### Code Style

- **Rust**: Follow standard `rustfmt` formatting. Run `cargo fmt` before committing.
- **Frontend**: Follow existing SvelteKit/TypeScript patterns in the codebase.
- **Commits**: Use [Conventional Commits](https://www.conventionalcommits.org/) format:
  ```
  feat(search): add infinite scroll to results
  fix(downloads): prevent duplicate entries
  docs: update installation guide
  ```

### Testing

```bash
# Backend tests
cd backend
cargo test

# Frontend type checking
cd frontend
npm run check
```

## Submitting Changes

1. **Push** your branch to your fork
2. **Open a Pull Request** against `main`
3. **Describe** what you changed and why
4. **Link** any related issues

## Reporting Issues

- Use [GitHub Issues](https://github.com/duytran1406/flasharr/issues)
- Include steps to reproduce, expected vs actual behavior, and logs if applicable
- Check existing issues before creating a new one

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
