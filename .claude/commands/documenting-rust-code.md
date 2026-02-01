---
description: Best practices for documenting Rust code with rustdoc
---

# Documenting Rust Code Guide

Use this skill when writing documentation for Rust code. Covers rustdoc conventions, doc comments, and documentation best practices.

## Documentation Comments

### Item Documentation (`///`)

```rust
/// Creates a new `User` with the given name and email.
///
/// # Arguments
///
/// * `name` - The user's display name
/// * `email` - The user's email address (must be valid format)
///
/// # Returns
///
/// A new `User` instance with a generated unique ID.
///
/// # Examples
///
/// ```
/// use my_crate::User;
///
/// let user = User::new("Alice", "alice@example.com");
/// assert_eq!(user.name(), "Alice");
/// ```
///
/// # Panics
///
/// Panics if `email` is empty.
///
/// # Errors
///
/// This function doesn't return errors, but see [`User::try_new`]
/// for a fallible version.
pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
    // implementation
}
```

### Module Documentation (`//!`)

```rust
//! # User Management Module
//!
//! This module provides types and functions for managing users in the system.
//!
//! ## Overview
//!
//! The main types are:
//! - [`User`] - Represents a user account
//! - [`UserRepository`] - Handles user persistence
//!
//! ## Examples
//!
//! ```
//! use my_crate::users::{User, UserRepository};
//!
//! let repo = UserRepository::new();
//! let user = User::new("Alice", "alice@example.com");
//! repo.save(&user);
//! ```
//!
//! ## Feature Flags
//!
//! - `async` - Enables async methods on `UserRepository`
//! - `serde` - Enables serialization support for `User`

mod user;
mod repository;

pub use user::User;
pub use repository::UserRepository;
```

## Documentation Sections

### Standard Sections (in order)

```rust
/// Brief one-line description.
///
/// More detailed explanation that can span multiple paragraphs.
/// This should explain what the item does and why you'd use it.
///
/// # Arguments
///
/// * `arg1` - Description of first argument
/// * `arg2` - Description of second argument
///
/// # Returns
///
/// Description of the return value.
///
/// # Examples
///
/// ```
/// // Example code that compiles and runs
/// ```
///
/// # Panics
///
/// Describe conditions that cause panics.
///
/// # Errors
///
/// Describe error conditions for Result-returning functions.
///
/// # Safety
///
/// For unsafe functions, explain the safety requirements.
///
/// # See Also
///
/// * [`related_function`] - For similar functionality
/// * [`OtherType`] - Related type
```

## Code Examples

### Runnable Examples

```rust
/// Parses a string into a number.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use my_crate::parse_number;
///
/// let num = parse_number("42").unwrap();
/// assert_eq!(num, 42);
/// ```
///
/// Handling errors:
///
/// ```
/// use my_crate::parse_number;
///
/// let result = parse_number("not a number");
/// assert!(result.is_err());
/// ```
pub fn parse_number(s: &str) -> Result<i32, ParseError> {
    s.parse().map_err(|_| ParseError::InvalidFormat)
}
```

### Non-Runnable Examples

```rust
/// Connects to the database.
///
/// # Examples
///
/// ```no_run
/// use my_crate::Database;
///
/// // This won't actually run during tests
/// let db = Database::connect("postgres://localhost/mydb").await?;
/// ```
pub async fn connect(url: &str) -> Result<Self, DbError> {
    // ...
}
```

### Examples That Should Panic

```rust
/// Divides two numbers.
///
/// # Panics
///
/// Panics if `divisor` is zero.
///
/// # Examples
///
/// ```should_panic
/// use my_crate::divide;
///
/// // This will panic
/// divide(10, 0);
/// ```
pub fn divide(dividend: i32, divisor: i32) -> i32 {
    if divisor == 0 {
        panic!("division by zero");
    }
    dividend / divisor
}
```

### Compile-Only Examples

```rust
/// Platform-specific function.
///
/// # Examples
///
/// ```compile_fail
/// // This intentionally fails to compile to demonstrate the API
/// use my_crate::WindowsOnly;
///
/// #[cfg(not(windows))]
/// let x = WindowsOnly::new(); // Error: not available on this platform
/// ```
```

### Hiding Lines

```rust
/// Creates a configured client.
///
/// # Examples
///
/// ```
/// # use my_crate::Client;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = Client::builder()
///     .timeout(30)
///     .build()?;
/// # Ok(())
/// # }
/// ```
```

## Linking

### Intra-doc Links

```rust
/// A user in the system.
///
/// Use [`User::new`] to create a new user.
/// See also [`UserRepository`] for persistence.
///
/// For batch operations, see the [`batch`] module.
///
/// Related traits: [`Authenticatable`], [`Serializable`]
pub struct User {
    // ...
}

/// Repository for [`User`] entities.
///
/// Implements the repository pattern described in
/// [`crate::patterns::Repository`].
pub struct UserRepository {
    // ...
}
```

### External Links

```rust
/// Implements the [Builder pattern].
///
/// See the [Rust API Guidelines] for more on builder patterns.
///
/// [Builder pattern]: https://rust-unofficial.github.io/patterns/patterns/creational/builder.html
/// [Rust API Guidelines]: https://rust-lang.github.io/api-guidelines/
pub struct ClientBuilder {
    // ...
}
```

## Struct and Enum Documentation

### Struct Fields

```rust
/// A configuration for the HTTP client.
///
/// # Examples
///
/// ```
/// use my_crate::ClientConfig;
///
/// let config = ClientConfig {
///     timeout_ms: 5000,
///     max_retries: 3,
///     base_url: "https://api.example.com".into(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Request timeout in milliseconds.
    ///
    /// Default is 30000 (30 seconds).
    pub timeout_ms: u64,

    /// Maximum number of retry attempts.
    ///
    /// Set to 0 to disable retries.
    pub max_retries: u32,

    /// Base URL for all requests.
    ///
    /// Must include the protocol (http:// or https://).
    pub base_url: String,
}
```

### Enum Variants

```rust
/// The status of a task.
///
/// Tasks progress through these states in order:
/// `Pending` → `Running` → `Completed` (or `Failed`)
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    /// Task is waiting to be executed.
    ///
    /// New tasks start in this state.
    Pending,

    /// Task is currently being executed.
    ///
    /// Contains the timestamp when execution started.
    Running {
        /// When the task started running.
        started_at: DateTime<Utc>,
    },

    /// Task completed successfully.
    ///
    /// Contains the result of the task.
    Completed {
        /// The task's output.
        result: String,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    /// Task failed with an error.
    Failed {
        /// Error message describing the failure.
        error: String,
    },
}
```

## Trait Documentation

```rust
/// A type that can be serialized to bytes.
///
/// # Implementing
///
/// Implement this trait for types that need binary serialization:
///
/// ```
/// use my_crate::Serializable;
///
/// struct Point { x: i32, y: i32 }
///
/// impl Serializable for Point {
///     fn to_bytes(&self) -> Vec<u8> {
///         let mut bytes = Vec::new();
///         bytes.extend_from_slice(&self.x.to_le_bytes());
///         bytes.extend_from_slice(&self.y.to_le_bytes());
///         bytes
///     }
/// }
/// ```
///
/// # Provided Methods
///
/// The [`to_hex`](Self::to_hex) method is provided by default
/// and converts the bytes to a hex string.
pub trait Serializable {
    /// Converts this value to a byte vector.
    ///
    /// The format is implementation-defined.
    fn to_bytes(&self) -> Vec<u8>;

    /// Converts this value to a hexadecimal string.
    ///
    /// Uses lowercase hex digits.
    fn to_hex(&self) -> String {
        self.to_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}
```

## Module-Level Documentation

```rust
// src/lib.rs

#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

//! # Additional API Documentation
//!
//! This section supplements the README with API-specific details.

pub mod users;
pub mod auth;
pub mod database;
```

## Crate-Level Settings

```rust
// At the top of lib.rs

#![doc(html_root_url = "https://docs.rs/my-crate/0.1.0")]
#![doc(html_favicon_url = "https://example.com/favicon.ico")]
#![doc(html_logo_url = "https://example.com/logo.png")]

// Warn on missing docs
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::private_intra_doc_links)]
```

## Documentation Testing

```bash
# Run doc tests
cargo test --doc

# Build docs and check for warnings
cargo doc --no-deps 2>&1 | grep -i warning

# Open docs in browser
cargo doc --open

# Check for broken links
cargo doc --no-deps 2>&1 | grep "unresolved link"
```

## Best Practices

1. **First line is a summary** - Shows in search results and module lists
2. **Use examples liberally** - They're tested by `cargo test`
3. **Document all public items** - Use `#![warn(missing_docs)]`
4. **Link to related items** - Use intra-doc links `[`Item`]`
5. **Document panics and errors** - Users need to know failure modes
6. **Keep examples minimal** - Show the API, not complex logic
7. **Use `# Safety` for unsafe** - Explain invariants that must be upheld
8. **Hide boilerplate in examples** - Use `#` to hide setup code
9. **Include "See Also" links** - Help users discover related APIs
10. **Update docs with code** - Outdated docs are worse than none
