# Contributing to Kibana Object Manager

Thank you for your interest in contributing to `kibob`! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Development Workflow](#development-workflow)
- [Code Style](#code-style)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)

---

## Code of Conduct

### Our Pledge

We pledge to make participation in our project a harassment-free experience for everyone, regardless of age, body size, disability, ethnicity, gender identity and expression, level of experience, nationality, personal appearance, race, religion, or sexual identity and orientation.

### Our Standards

**Examples of behavior that contributes to creating a positive environment:**
- Using welcoming and inclusive language
- Being respectful of differing viewpoints and experiences
- Gracefully accepting constructive criticism
- Focusing on what is best for the community
- Showing empathy towards other community members

**Examples of unacceptable behavior:**
- Trolling, insulting/derogatory comments, and personal or political attacks
- Public or private harassment
- Publishing others' private information without explicit permission
- Other conduct which could reasonably be considered inappropriate in a professional setting

### Enforcement

Instances of abusive, harassing, or otherwise unacceptable behavior may be reported by opening an issue or contacting the project maintainer at [GitHub](https://github.com/VimCommando). All complaints will be reviewed and investigated promptly and fairly.

---

## Getting Started

### Prerequisites

- **Rust** 1.89 or higher
- **Git** for version control
- **Kibana** 8.x for integration testing (optional)
- **Docker** for running test Kibana instance (optional)

### Ways to Contribute

We welcome contributions in many forms:

- **Bug reports** - Found a bug? Open an issue with detailed reproduction steps
- **Feature requests** - Have an idea? Create an issue to discuss it
- **Documentation** - Improve docs, fix typos, add examples
- **Code** - Fix bugs, implement features, improve performance
- **Testing** - Write tests, improve test coverage
- **Review** - Review pull requests, provide feedback

---

## Development Setup

### 1. Fork and Clone

```bash
# Fork the repository on GitHub, then:
git clone https://github.com/YOUR_USERNAME/kibana-object-manager.git
cd kibana-object-manager

# Add upstream remote
git remote add upstream https://github.com/VimCommando/kibana-object-manager.git
```

### 2. Install Dependencies

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Update Rust to latest stable
rustup update stable

# Install development tools
rustup component add rustfmt clippy
```

### 3. Build the Project

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run
./target/debug/kibob --help
```

### 4. Run Tests

```bash
# Run all tests
cargo test --all

# Run with verbose output
cargo test --all -- --nocapture

# Run specific test
cargo test test_field_dropper

# Run integration tests only
cargo test --test '*'
```

### 5. Set Up Test Environment (Optional)

For integration testing with real Kibana:

```bash
# Start Kibana with Docker Compose
docker-compose up -d

# Wait for Kibana to be ready
until curl -s http://localhost:5601/api/status | grep -q "available"; do
  sleep 2
done

# Set environment variables
export KIBANA_URL=http://localhost:5601
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme

# Run integration tests
cargo test --test saved_objects_integration -- --ignored
```

---

## Development Workflow

### Branch Naming

Use descriptive branch names:

- `feature/add-status-command` - New features
- `fix/handle-empty-manifest` - Bug fixes
- `docs/improve-quickstart` - Documentation
- `refactor/simplify-pipeline` - Code refactoring
- `test/add-integration-tests` - Test improvements

### Commit Messages

Follow conventional commit format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation changes
- `style` - Code style (formatting, whitespace)
- `refactor` - Code refactoring
- `test` - Test additions or changes
- `chore` - Build, CI, or tooling changes

**Examples:**
```bash
feat(cli): add validate command

Add new validate command to check project structure
and manifest consistency before deployment.

Closes #42

---

fix(transform): handle null values in field escaper

The field escaper was panicking on null values in nested
JSON objects. Now properly handles nulls by skipping them.

Fixes #38

---

docs(examples): add multi-environment deployment guide

Added comprehensive example showing how to deploy dashboards
across dev, staging, and production environments.
```

### Keep Your Fork Updated

```bash
# Fetch upstream changes
git fetch upstream

# Merge into your main branch
git checkout main
git merge upstream/main

# Push to your fork
git push origin main
```

---

## Code Style

### Rust Style Guide

We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).

### Formatting

Format your code before committing:

```bash
# Format all code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check
```

### Linting

Run Clippy for code quality checks:

```bash
# Run clippy
cargo clippy --all-targets --all-features

# Deny all warnings (CI does this)
cargo clippy --all-targets --all-features -- -D warnings
```

### Code Organization

**Module structure:**
```rust
// Public API at top
pub struct MyStruct { ... }
pub fn public_function() { ... }

// Private helpers below
fn private_helper() { ... }

// Tests at bottom
#[cfg(test)]
mod tests { ... }
```

**Error handling:**
```rust
// Use eyre::Result for fallible functions
use eyre::Result;

pub fn may_fail() -> Result<String> {
    let value = std::fs::read_to_string("file.txt")?;
    Ok(value)
}

// Provide context for errors
.wrap_err("Failed to read configuration file")?
```

**Async functions:**
```rust
// Use async_trait for trait methods
#[async_trait]
pub trait Extractor {
    async fn extract(&self) -> Result<Vec<Value>>;
}

// Prefer async/await over futures combinators
async fn fetch_data() -> Result<String> {
    let response = client.get(url).send().await?;
    let text = response.text().await?;
    Ok(text)
}
```

### Documentation

Document public APIs with doc comments:

```rust
/// Extracts saved objects from Kibana.
///
/// # Arguments
///
/// * `client` - Kibana HTTP client
/// * `manifest` - List of objects to extract
///
/// # Returns
///
/// Vector of saved objects as JSON values
///
/// # Errors
///
/// Returns error if Kibana API call fails or response is invalid
///
/// # Example
///
/// ```
/// let extractor = SavedObjectsExtractor::new(client, manifest);
/// let objects = extractor.extract().await?;
/// ```
pub async fn extract(&self) -> Result<Vec<Value>> {
    // ...
}
```

---

## Testing

### Writing Tests

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_dropper_removes_field() {
        let dropper = FieldDropper::new(vec!["managed"]);
        let input = json!({"id": "abc", "managed": true});
        let output = dropper.drop_field(input);
        assert_eq!(output, json!({"id": "abc"}));
    }

    #[test]
    fn test_field_dropper_preserves_other_fields() {
        let dropper = FieldDropper::new(vec!["managed"]);
        let input = json!({"id": "abc", "title": "Test", "managed": true});
        let output = dropper.drop_field(input);
        assert!(output.get("title").is_some());
    }
}
```

#### Async Tests

```rust
#[tokio::test]
async fn test_extractor_fetches_objects() {
    let client = create_test_client();
    let manifest = create_test_manifest();
    
    let extractor = SavedObjectsExtractor::new(client, manifest);
    let result = extractor.extract().await;
    
    assert!(result.is_ok());
    let objects = result.unwrap();
    assert_eq!(objects.len(), 2);
}
```

#### Integration Tests

```rust
// tests/integration_test.rs
use kibana_object_manager::*;
use tempfile::TempDir;

#[tokio::test]
#[ignore] // Only run with --ignored flag
async fn test_roundtrip_with_real_kibana() {
    // Requires KIBANA_URL environment variable
    let temp_dir = TempDir::new().unwrap();
    
    // Pull from Kibana
    pull_saved_objects(temp_dir.path()).await.unwrap();
    
    // Verify files created
    assert!(temp_dir.path().join("objects").exists());
    
    // Push back
    push_saved_objects(temp_dir.path(), true).await.unwrap();
}
```

### Test Coverage

Check test coverage:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# Open report
open coverage/index.html
```

**Target: 85%+ code coverage**

---

## Pull Request Process

### Before Submitting

1. **Run all checks:**
   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all
   cargo build --release
   ```

2. **Update documentation:**
   - Update README if adding user-facing features
   - Update CHANGELOG.md with your changes
   - Add/update doc comments for new public APIs
   - Add examples to docs/ if appropriate

3. **Add tests:**
   - Unit tests for new functions
   - Integration tests for new commands
   - Update existing tests if changing behavior

### Submitting Pull Request

1. **Push to your fork:**
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Open pull request on GitHub:**
   - Use a clear, descriptive title
   - Reference related issues (e.g., "Fixes #123")
   - Describe what changed and why
   - Include test results
   - Add screenshots for UI changes

3. **Pull request template:**
   ```markdown
   ## Description
   Brief description of changes
   
   ## Related Issues
   Fixes #123
   Closes #456
   
   ## Changes Made
   - Added new `validate` command
   - Updated manifest schema to support validation
   - Added integration tests
   
   ## Testing
   - [ ] Unit tests pass
   - [ ] Integration tests pass
   - [ ] Manual testing completed
   
   ## Checklist
   - [ ] Code formatted with `cargo fmt`
   - [ ] No clippy warnings
   - [ ] Documentation updated
   - [ ] CHANGELOG.md updated
   - [ ] Tests added/updated
   ```

### Review Process

1. **Automated checks** - CI will run tests and linting
2. **Maintainer review** - Code review by project maintainer
3. **Feedback** - Address any requested changes
4. **Approval** - Maintainer approves PR
5. **Merge** - Maintainer merges to main

### After Merge

- Your contribution will be included in the next release
- You'll be credited in CHANGELOG.md
- Thank you for contributing!

---

## Release Process

*For maintainers*

### Version Numbering

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR** (1.0.0) - Breaking changes
- **MINOR** (0.1.0) - New features, backwards compatible
- **PATCH** (0.0.1) - Bug fixes, backwards compatible

### Release Steps

1. **Update version in Cargo.toml:**
   ```toml
   [package]
   version = "0.2.0"
   ```

2. **Update CHANGELOG.md:**
   ```markdown
   ## [0.2.0] - 2026-01-XX
   
   ### Added
   - New validate command
   - Support for Canvas workpads
   
   ### Changed
   - Improved error messages
   
   ### Fixed
   - Handle null values in transformers
   ```

3. **Commit and tag:**
   ```bash
   git add Cargo.toml CHANGELOG.md
   git commit -m "chore: bump version to 0.2.0"
   git tag -a v0.2.0 -m "Release version 0.2.0"
   git push origin main --tags
   ```

4. **Publish to crates.io:**
   ```bash
   cargo publish --dry-run
   cargo publish
   ```

5. **Create GitHub release:**
   - Go to GitHub → Releases → Draft new release
   - Select tag v0.2.0
   - Copy CHANGELOG entry as release notes
   - Publish release

6. **Announce:**
   - Update README with new version
   - Post in discussions
   - Share on social media (if applicable)

---

## Questions?

- **General questions**: Open a [Discussion](https://github.com/VimCommando/kibana-object-manager/discussions)
- **Bug reports**: Open an [Issue](https://github.com/VimCommando/kibana-object-manager/issues)
- **Security concerns**: Email maintainer directly (see GitHub profile)

---

## License

By contributing to Kibana Object Manager, you agree that your contributions will be licensed under the Apache License 2.0.

---

## Acknowledgments

Thank you to all contributors who help make kibob better!

**Contributors:**
- Ryan Eno ([@VimCommando](https://github.com/VimCommando)) - Original author

*Your name here! Submit a PR to get started.*
