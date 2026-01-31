---
description: Modern Rust development patterns, idioms, and best practices
---

# Rust Development Guide

Use this skill for Rust development. Covers ownership, error handling, async patterns, and idiomatic Rust.

## Project Structure

```
my_crate/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Library entry point
│   ├── main.rs         # Binary entry point (if applicable)
│   ├── error.rs        # Error types
│   ├── types.rs        # Shared types
│   └── module/
│       ├── mod.rs      # Module entry
│       └── impl.rs     # Implementation
├── tests/              # Integration tests
├── benches/            # Benchmarks
└── examples/           # Example code
```

## Ownership & Borrowing

### Core Rules

```rust
// 1. Each value has exactly one owner
let s1 = String::from("hello");
let s2 = s1;  // s1 is moved, no longer valid

// 2. Borrowing: & for read, &mut for write
fn read(s: &String) { println!("{}", s); }
fn modify(s: &mut String) { s.push_str("!"); }

// 3. Either one &mut OR any number of &, never both
let mut s = String::from("hello");
let r1 = &s;      // OK
let r2 = &s;      // OK
// let r3 = &mut s; // ERROR: can't borrow as mutable

// 4. References must be valid (no dangling)
```

### Clone vs Copy

```rust
// Copy: stack-only, implicit copy (i32, bool, char, tuples of Copy types)
let x = 5;
let y = x;  // x is still valid, copied

// Clone: explicit deep copy
let s1 = String::from("hello");
let s2 = s1.clone();  // Both valid, heap data copied

// Prefer borrowing over cloning when possible
fn process(data: &[u8]) -> usize { data.len() }
```

## Error Handling

### Result and Option

```rust
// Result for recoverable errors
fn read_file(path: &str) -> Result<String, io::Error> {
    std::fs::read_to_string(path)
}

// Option for optional values
fn find_user(id: u64) -> Option<User> {
    users.iter().find(|u| u.id == id).cloned()
}

// Combinators
let content = read_file("config.toml")
    .map(|s| s.to_uppercase())
    .unwrap_or_default();

let name = find_user(42)
    .map(|u| u.name)
    .unwrap_or_else(|| "Anonymous".to_string());
```

### The ? Operator

```rust
fn process_config() -> Result<Config, Box<dyn Error>> {
    let content = std::fs::read_to_string("config.toml")?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
```

### Custom Error Types

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {source}")]
    Database {
        #[from]
        source: sqlx::Error,
    },

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("IO error")]
    Io(#[from] std::io::Error),
}

// Usage
fn load_config() -> Result<Config, AppError> {
    let path = std::env::var("CONFIG_PATH")
        .map_err(|_| AppError::Config("CONFIG_PATH not set".into()))?;
    let content = std::fs::read_to_string(&path)?;  // Auto-converts io::Error
    Ok(toml::from_str(&content)?)
}
```

## Structs and Traits

### Struct Patterns

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    created_at: DateTime<Utc>,  // private field
}

impl User {
    // Constructor
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            id: generate_id(),
            name: name.into(),
            email: email.into(),
            created_at: Utc::now(),
        }
    }

    // Getter for private field
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    // Builder pattern
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }
}
```

### Trait Patterns

```rust
// Define behavior
pub trait Repository<T> {
    fn get(&self, id: &str) -> Option<T>;
    fn save(&mut self, item: T) -> Result<(), Error>;
    fn delete(&mut self, id: &str) -> Result<(), Error>;
}

// Implement for concrete type
impl Repository<User> for PostgresRepo {
    fn get(&self, id: &str) -> Option<User> {
        self.pool.query_one("SELECT * FROM users WHERE id = $1", &[&id])
            .ok()
            .map(|row| User::from(row))
    }

    fn save(&mut self, user: User) -> Result<(), Error> {
        // ...
    }

    fn delete(&mut self, id: &str) -> Result<(), Error> {
        // ...
    }
}

// Trait objects for runtime polymorphism
fn use_repo(repo: &dyn Repository<User>) {
    if let Some(user) = repo.get("123") {
        println!("{:?}", user);
    }
}

// Generics for compile-time polymorphism (preferred when possible)
fn use_repo_generic<R: Repository<User>>(repo: &R) {
    if let Some(user) = repo.get("123") {
        println!("{:?}", user);
    }
}
```

## Async Patterns

### Basic Async

```rust
use tokio;

#[tokio::main]
async fn main() {
    let result = fetch_data().await;
    println!("{:?}", result);
}

async fn fetch_data() -> Result<Data, Error> {
    let response = reqwest::get("https://api.example.com/data").await?;
    let data: Data = response.json().await?;
    Ok(data)
}
```

### Concurrent Execution

```rust
use tokio::join;
use futures::future::join_all;

// Run multiple futures concurrently
async fn fetch_all() -> Result<(User, Posts, Comments), Error> {
    let (user, posts, comments) = join!(
        fetch_user(),
        fetch_posts(),
        fetch_comments()
    );
    Ok((user?, posts?, comments?))
}

// Dynamic number of futures
async fn fetch_many(ids: Vec<u64>) -> Vec<Result<Data, Error>> {
    let futures: Vec<_> = ids.iter().map(|id| fetch_one(*id)).collect();
    join_all(futures).await
}
```

### Shared State in Async

```rust
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};

#[derive(Clone)]
struct AppState {
    db: Arc<Pool>,
    cache: Arc<RwLock<HashMap<String, Data>>>,
    counter: Arc<Mutex<u64>>,
}

impl AppState {
    async fn get_cached(&self, key: &str) -> Option<Data> {
        self.cache.read().await.get(key).cloned()
    }

    async fn set_cached(&self, key: String, data: Data) {
        self.cache.write().await.insert(key, data);
    }

    async fn increment(&self) -> u64 {
        let mut counter = self.counter.lock().await;
        *counter += 1;
        *counter
    }
}
```

### Spawning Tasks

```rust
use tokio::spawn;
use tokio::sync::mpsc;

async fn producer_consumer() {
    let (tx, mut rx) = mpsc::channel(100);

    // Spawn producer
    let producer = spawn(async move {
        for i in 0..100 {
            tx.send(i).await.unwrap();
        }
    });

    // Spawn consumer
    let consumer = spawn(async move {
        while let Some(value) = rx.recv().await {
            println!("Received: {}", value);
        }
    });

    // Wait for both
    let _ = tokio::join!(producer, consumer);
}
```

## Iterators

```rust
// Chaining iterators
let result: Vec<_> = items
    .iter()
    .filter(|item| item.is_active())
    .map(|item| item.name.clone())
    .take(10)
    .collect();

// Fold for accumulation
let sum: i32 = numbers.iter().fold(0, |acc, x| acc + x);

// Find and find_map
let first_admin = users.iter().find(|u| u.is_admin());
let admin_email = users.iter().find_map(|u| {
    if u.is_admin() { Some(u.email.clone()) } else { None }
});

// Partition
let (active, inactive): (Vec<_>, Vec<_>) = users
    .into_iter()
    .partition(|u| u.is_active());
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let result = add(2, 3);
        assert_eq!(result, 5);
    }

    #[test]
    fn test_with_setup() {
        let user = User::new("Alice", "alice@example.com");
        assert!(!user.name.is_empty());
        assert!(user.email.contains("@"));
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn test_panic() {
        divide(10, 0);
    }

    #[tokio::test]
    async fn test_async() {
        let result = fetch_data().await;
        assert!(result.is_ok());
    }
}

// Integration tests in tests/ directory
// tests/integration_test.rs
use my_crate::process;

#[test]
fn test_full_workflow() {
    let input = setup_test_data();
    let output = process(input);
    assert_eq!(output.status, "completed");
}
```

## Common Idioms

### Entry API for Maps

```rust
use std::collections::HashMap;

let mut counts: HashMap<&str, u32> = HashMap::new();

// Insert or update
*counts.entry("key").or_insert(0) += 1;

// Insert with computation
counts.entry("key").or_insert_with(|| expensive_computation());
```

### Type Conversions

```rust
// From/Into for infallible conversions
impl From<Row> for User {
    fn from(row: Row) -> Self {
        User {
            id: row.get("id"),
            name: row.get("name"),
        }
    }
}

let user: User = row.into();

// TryFrom/TryInto for fallible conversions
impl TryFrom<&str> for Status {
    type Error = ParseError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "active" => Ok(Status::Active),
            "inactive" => Ok(Status::Inactive),
            _ => Err(ParseError::InvalidStatus),
        }
    }
}
```

### Deref for Smart Pointers

```rust
use std::ops::Deref;

struct MyBox<T>(T);

impl<T> Deref for MyBox<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

let boxed = MyBox(String::from("hello"));
println!("{}", boxed.len());  // Deref coercion to &String
```

## Performance Tips

1. **Avoid unnecessary allocations**: Use `&str` instead of `String` when possible
2. **Use iterators**: They're often zero-cost abstractions
3. **Prefer stack allocation**: Small, known-size types on the stack
4. **Use `Cow<str>`**: For maybe-owned strings
5. **Profile first**: Use `cargo flamegraph` or `perf` before optimizing

```rust
use std::borrow::Cow;

fn process_name(name: Cow<str>) -> String {
    if name.contains(' ') {
        name.replace(' ', "_")
    } else {
        name.into_owned()
    }
}

// Can call with &str (no allocation) or String
process_name(Cow::Borrowed("hello"));
process_name(Cow::Owned(String::from("hello")));
```

## Common Pitfalls

1. **Fighting the borrow checker**: Restructure code instead
2. **Overusing `clone()`**: Usually indicates design issue
3. **Ignoring lifetimes**: Understand them, don't just add `'static`
4. **Not using `?` operator**: Cleaner than manual error handling
5. **Blocking in async**: Use `spawn_blocking` for CPU-intensive work
6. **Mutex poisoning**: Handle `PoisonError` appropriately
