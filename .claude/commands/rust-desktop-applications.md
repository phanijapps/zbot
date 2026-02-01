---
description: Patterns for building desktop applications in Rust with GUI frameworks
---

# Rust Desktop Applications Guide

Use this skill when building desktop applications in Rust. Covers architecture, GUI frameworks, and desktop-specific patterns.

## Framework Options

### Tauri (Web-based UI)
- Web frontend (React, Vue, Svelte) + Rust backend
- Small binary size, native system APIs
- Best for: Web developers, cross-platform apps

### Iced (Pure Rust)
- Elm-like architecture, purely functional
- Cross-platform, GPU-accelerated
- Best for: Pure Rust projects, reactive UIs

### egui (Immediate Mode)
- Simple, fast prototyping
- Game-dev style rendering
- Best for: Tools, debug UIs, games

### GTK4-rs
- Native look on Linux
- Full-featured widget library
- Best for: Linux-focused apps

## Tauri Architecture

```
my-app/
├── src-tauri/          # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json # App configuration
│   └── src/
│       ├── main.rs     # Entry point
│       ├── commands.rs # IPC commands
│       └── state.rs    # App state
├── src/                # Frontend (React/Vue/etc)
│   ├── App.tsx
│   └── ...
└── package.json
```

### Tauri Commands

```rust
use tauri::{command, State};
use std::sync::Mutex;

struct AppState {
    counter: Mutex<u32>,
}

#[command]
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

#[command]
async fn fetch_data(url: String) -> Result<String, String> {
    reqwest::get(&url)
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

#[command]
fn increment(state: State<AppState>) -> u32 {
    let mut counter = state.counter.lock().unwrap();
    *counter += 1;
    *counter
}

fn main() {
    tauri::Builder::default()
        .manage(AppState { counter: Mutex::new(0) })
        .invoke_handler(tauri::generate_handler![greet, fetch_data, increment])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Frontend Integration (TypeScript)

```typescript
import { invoke } from "@tauri-apps/api/core";

// Call Rust command
const greeting = await invoke<string>("greet", { name: "World" });

// With error handling
try {
  const data = await invoke<string>("fetch_data", { url: "https://api.example.com" });
  console.log(data);
} catch (error) {
  console.error("Failed:", error);
}
```

## Iced Architecture

```rust
use iced::{Application, Command, Element, Settings, Theme};
use iced::widget::{button, column, text, text_input};

#[derive(Default)]
struct Counter {
    value: i32,
    input: String,
}

#[derive(Debug, Clone)]
enum Message {
    Increment,
    Decrement,
    InputChanged(String),
    Submit,
}

impl Application for Counter {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (Self::default(), Command::none())
    }

    fn title(&self) -> String {
        String::from("Counter")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
            Message::InputChanged(value) => self.input = value,
            Message::Submit => {
                if let Ok(n) = self.input.parse::<i32>() {
                    self.value = n;
                    self.input.clear();
                }
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        column![
            button("-").on_press(Message::Decrement),
            text(self.value).size(50),
            button("+").on_press(Message::Increment),
            text_input("Enter value...", &self.input)
                .on_input(Message::InputChanged)
                .on_submit(Message::Submit),
        ]
        .padding(20)
        .into()
    }
}

fn main() -> iced::Result {
    Counter::run(Settings::default())
}
```

## State Management

### Application State

```rust
use std::sync::{Arc, Mutex, RwLock};

// For read-heavy workloads
pub struct AppState {
    pub config: RwLock<Config>,
    pub cache: RwLock<HashMap<String, Data>>,
}

// Thread-safe mutable state
pub struct MutableState {
    inner: Arc<Mutex<StateInner>>,
}

impl MutableState {
    pub fn update<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut StateInner) -> R,
    {
        let mut guard = self.inner.lock().unwrap();
        f(&mut *guard)
    }
}
```

### Event-Driven Updates

```rust
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
enum AppEvent {
    DataUpdated(String),
    SettingsChanged,
    Error(String),
}

struct EventBus {
    sender: broadcast::Sender<AppEvent>,
}

impl EventBus {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    fn publish(&self, event: AppEvent) {
        let _ = self.sender.send(event);
    }

    fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.sender.subscribe()
    }
}
```

## File System Operations

```rust
use std::path::PathBuf;
use directories::ProjectDirs;

fn get_app_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "MyCompany", "MyApp")
}

fn get_config_path() -> PathBuf {
    get_app_dirs()
        .map(|dirs| dirs.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn get_data_path() -> PathBuf {
    get_app_dirs()
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

// Platform-specific paths
#[cfg(target_os = "windows")]
fn get_documents() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(target_os = "macos")]
fn get_documents() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(target_os = "linux")]
fn get_documents() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| PathBuf::from("."))
}
```

## Native System Integration

### System Tray

```rust
use tauri::{CustomMenuItem, SystemTray, SystemTrayMenu, SystemTrayEvent};

fn create_tray() -> SystemTray {
    let menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("show", "Show"))
        .add_item(CustomMenuItem::new("quit", "Quit"));

    SystemTray::new().with_menu(menu)
}

fn handle_tray_event(app: &tauri::AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
            "show" => {
                if let Some(window) = app.get_window("main") {
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
            }
            "quit" => std::process::exit(0),
            _ => {}
        },
        SystemTrayEvent::LeftClick { .. } => {
            // Show on left click
        }
        _ => {}
    }
}
```

### Notifications

```rust
use notify_rust::Notification;

fn show_notification(title: &str, body: &str) -> Result<(), notify_rust::error::Error> {
    Notification::new()
        .summary(title)
        .body(body)
        .timeout(5000)
        .show()?;
    Ok(())
}
```

### Clipboard

```rust
use arboard::Clipboard;

fn copy_to_clipboard(text: &str) -> Result<(), arboard::Error> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;
    Ok(())
}

fn paste_from_clipboard() -> Result<String, arboard::Error> {
    let mut clipboard = Clipboard::new()?;
    clipboard.get_text()
}
```

## Window Management

```rust
use tauri::{Manager, WindowBuilder, WindowUrl};

// Create new window
fn create_settings_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    WindowBuilder::new(
        app,
        "settings",
        WindowUrl::App("settings.html".into())
    )
    .title("Settings")
    .inner_size(600.0, 400.0)
    .resizable(false)
    .build()?;
    Ok(())
}

// Window events
fn setup_window_events(window: &tauri::Window) {
    let window_clone = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // Prevent close, minimize to tray instead
            api.prevent_close();
            window_clone.hide().unwrap();
        }
        tauri::WindowEvent::Focused(focused) => {
            if *focused {
                // Window gained focus
            }
        }
        _ => {}
    });
}
```

## Background Tasks

```rust
use std::time::Duration;
use tokio::sync::mpsc;

async fn run_background_service(
    mut shutdown: mpsc::Receiver<()>,
    event_bus: Arc<EventBus>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Periodic task
                if let Err(e) = check_for_updates().await {
                    event_bus.publish(AppEvent::Error(e.to_string()));
                }
            }
            _ = shutdown.recv() => {
                // Graceful shutdown
                break;
            }
        }
    }
}
```

## Packaging & Distribution

### Cargo.toml for Desktop

```toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
directories = "5"
notify-rust = "4"
arboard = "3"

# Windows-specific
[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["winuser"] }

# macOS-specific
[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.25"

[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Link-time optimization
codegen-units = 1   # Better optimization
strip = true        # Strip symbols
```

### Cross-Platform Considerations

```rust
// Platform-specific code
#[cfg(target_os = "windows")]
fn platform_init() {
    // Windows-specific initialization
}

#[cfg(target_os = "macos")]
fn platform_init() {
    // macOS-specific initialization
}

#[cfg(target_os = "linux")]
fn platform_init() {
    // Linux-specific initialization
}

// Path separators
use std::path::MAIN_SEPARATOR;

// Line endings
#[cfg(windows)]
const LINE_ENDING: &str = "\r\n";

#[cfg(not(windows))]
const LINE_ENDING: &str = "\n";
```

## Debugging Tips

1. **Use `RUST_BACKTRACE=1`** for stack traces
2. **Enable debug logging** with `RUST_LOG=debug`
3. **Chrome DevTools** for Tauri frontend
4. **`cargo run --release`** to test release behavior
5. **Memory profiling** with `heaptrack` or Instruments (macOS)
