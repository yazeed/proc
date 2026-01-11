# Contributing to proc

Thank you for your interest in contributing to proc! This document provides guidelines and instructions for contributing.

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

## How to Contribute

### Reporting Bugs

Before creating a bug report, please check the [existing issues](https://github.com/yazeed/proc/issues) to see if the problem has already been reported.

When creating a bug report, please include:

1. **Environment details**:
   - Operating system and version
   - Rust version (`rustc --version`)
   - proc version (`proc --version`)

2. **Steps to reproduce**:
   - What commands did you run?
   - What did you expect to happen?
   - What actually happened?

3. **Additional context**:
   - Error messages (full output)
   - Screenshots if applicable

### Suggesting Features

Feature suggestions are welcome! Please create an issue with:

1. A clear description of the feature
2. Use cases and examples
3. Any alternatives you've considered

### Pull Requests

1. **Fork the repository** and create your branch from `main`

2. **Set up the development environment**:
   ```bash
   git clone https://github.com/yazeed/proc
   cd proc
   cargo build
   ```

3. **Make your changes**:
   - Write clear, concise commit messages
   - Add tests for new functionality
   - Update documentation as needed

4. **Run the test suite**:
   ```bash
   cargo test
   cargo fmt -- --check
   cargo clippy -- -D warnings
   ```

5. **Submit a pull request**:
   - Provide a clear description of the changes
   - Reference any related issues

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Git

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with verbose output
cargo test -- --nocapture
```

### Code Style

We follow standard Rust conventions:

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Run linter
cargo clippy -- -D warnings
```

### Project Structure

```
proc/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Library exports
│   ├── error.rs          # Error types
│   ├── commands/         # Command implementations
│   │   ├── mod.rs
│   │   ├── find.rs
│   │   ├── on.rs
│   │   ├── ports.rs
│   │   ├── kill.rs
│   │   └── stuck.rs
│   ├── core/             # Core abstractions
│   │   ├── mod.rs
│   │   ├── process.rs
│   │   └── port.rs
│   └── ui/               # Output formatting
│       ├── mod.rs
│       └── output.rs
├── tests/                # Integration tests
├── Cargo.toml
└── README.md
```

## Areas to Contribute

### Good First Issues

Look for issues labeled `good first issue` - these are great starting points:

- Documentation improvements
- Error message enhancements
- Test coverage improvements

### Medium Complexity

- Platform-specific bug fixes
- Performance optimizations
- Output formatting improvements

### Advanced

- New commands
- Cross-platform compatibility
- Advanced features (watch mode, process trees)

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_find_all_processes

# Run tests with output
cargo test -- --nocapture
```

### Writing Tests

- Unit tests go in the same file as the code being tested
- Integration tests go in the `tests/` directory
- Use descriptive test names

Example:
```rust
#[test]
fn test_find_process_by_name() {
    let processes = Process::find_by_name("cargo").unwrap();
    assert!(!processes.is_empty());
}
```

## Commit Messages

Follow conventional commit format:

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

Examples:
```
feat(commands): add proc watch command
fix(ports): handle IPv6 addresses correctly
docs: update installation instructions
```

## Release Process

Releases are managed by maintainers. The process:

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create a git tag
4. GitHub Actions builds and publishes releases

## Questions?

- Open a [Discussion](https://github.com/yazeed/proc/discussions) for questions
- Join our community chat (if available)
- Reach out to maintainers

Thank you for contributing to proc!
