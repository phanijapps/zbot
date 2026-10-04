use super::*;
use serde_json::{json, Value};
fn config(hooks: Value) -> Value {
    json!({"version":1,"hooks":hooks})
}
fn hook() -> Value {
    json!({"id":"audit","event":"run_start","command":["python3","config/hooks/audit.py"]})
}
fn vault() -> (tempfile::TempDir, VaultPaths) {
    let temp = tempfile::tempdir().unwrap();
    let paths = VaultPaths::new(temp.path().to_owned());
    paths.ensure_dirs_exist().unwrap();
    (temp, paths)
}
fn write_bytes(paths: &VaultPaths, bytes: &[u8]) {
    std::fs::write(paths.hooks_config(), bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(paths.hooks_config(), std::fs::Permissions::from_mode(0o600))
            .unwrap();
    }
}
fn write(paths: &VaultPaths, value: &Value) {
    write_bytes(paths, value.to_string().as_bytes());
}
#[tokio::test]
async fn absent_is_empty_and_defaults_follow_schema() {
    let (_temp, paths) = vault();
    let empty = HookSnapshot::load(&paths, &[]).await.unwrap();
    assert!(empty.hooks().is_empty());
    write(&paths, &config(json!([hook()])));
    let loaded = HookSnapshot::load(&paths, &[]).await.unwrap();
    let definition = &loaded.hooks()[0];
    assert!(definition.enabled);
    assert_eq!(definition.cwd, HookCwd::Vault);
    assert_eq!(definition.timeout_ms, 5000);
    assert_eq!(definition.on_failure, HookFailurePolicy::Continue);
    assert_eq!(definition.command, ["python3", "config/hooks/audit.py"]);
    assert_ne!(empty.revision(), loaded.revision());
}
#[tokio::test]
async fn invalid_configs_fail_closed() {
    let (_temp, paths) = vault();
    let mut invalid = vec![
        json!({"version":1,"hooks":[],"$schema":null}),
        json!({}),
        json!({"version":2,"hooks":[]}),
        json!({"version":"1","hooks":[]}),
        json!({"version":1,"hooks":[],"extra":true}),
        config(json!([hook(), hook()])),
    ];
    for (key, value) in [
        ("id", json!("Bad")),
        ("id", json!("a".repeat(65))),
        ("event", json!("unknown")),
        ("event", json!({"run_start":null})),
        ("cwd", json!({"vault":null})),
        ("on_failure", json!({"continue":null})),
        ("cwd", json!("ward")),
        ("timeout_ms", json!(0)),
        ("timeout_ms", json!(300001)),
        ("command", json!([])),
        ("command", json!(["x\u{0}y"])),
        ("command", json!([""])),
        ("command", json!(["a".repeat(4097)])),
        ("command", json!(vec!["arg"; 65])),
        ("unknown", json!(true)),
        ("enabled", json!("yes")),
    ] {
        let mut value_hook = hook();
        value_hook[key] = value;
        invalid.push(config(json!([value_hook])));
    }
    for event in ["run_end", "after_model", "after_tool"] {
        let mut h = hook();
        h["event"] = json!(event);
        h["on_failure"] = json!("block");
        invalid.push(config(json!([h])));
    }
    let many: Vec<_> = (0..101)
        .map(|i| {
            let mut h = hook();
            h["id"] = json!(format!("hook-{i}"));
            h
        })
        .collect();
    invalid.push(config(json!(many)));
    for value in invalid {
        write(&paths, &value);
        assert!(
            HookSnapshot::load(&paths, &[]).await.is_err(),
            "accepted {value}"
        );
    }
    std::fs::write(paths.hooks_config(), "not json secret-token").unwrap();
    let error = HookSnapshot::load(&paths, &[])
        .await
        .unwrap_err()
        .to_string();
    assert!(!error.contains("secret-token"));
    assert!(error.len() < 256);
}
#[tokio::test]
async fn snapshots_keep_order_and_identity_through_file_edits_and_failure() {
    let (_temp, paths) = vault();
    let mut disabled = hook();
    disabled["id"] = json!("disabled");
    disabled["enabled"] = json!(false);
    write(&paths, &config(json!([disabled, hook()])));
    let old = HookSnapshot::load(&paths, &[]).await.unwrap();
    let child = old.clone();
    assert!(Arc::ptr_eq(&old, &child));
    assert_eq!(old.hooks()[0].id, "disabled");
    assert!(!old.hooks()[0].enabled);
    let again = HookSnapshot::load(&paths, &[]).await.unwrap();
    assert_eq!(old.revision(), again.revision());
    write(&paths, &config(json!([])));
    let new = HookSnapshot::load(&paths, &[]).await.unwrap();
    assert!(new.hooks().is_empty());
    assert_eq!(child.hooks().len(), 2);
    std::fs::write(paths.hooks_config(), "broken").unwrap();
    assert!(HookSnapshot::load(&paths, &[]).await.is_err());
    assert_eq!(old.hooks().len(), 2);
}
#[tokio::test]
async fn exact_size_limit_is_enforced_before_parsing() {
    let (_temp, paths) = vault();
    let mut bytes = b"{\"version\":1,\"hooks\":[]}".to_vec();
    bytes.resize(MAX_CONFIG_BYTES, b' ');
    write_bytes(&paths, &bytes);
    assert!(HookSnapshot::load(&paths, &[]).await.is_ok());
    bytes.push(b' ');
    write_bytes(&paths, &bytes);
    assert_eq!(
        HookSnapshot::load(&paths, &[]).await.unwrap_err(),
        HookConfigError::TooLarge
    );
}
#[cfg(unix)]
#[tokio::test]
async fn writable_and_project_symlink_config_are_rejected() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let (_temp, paths) = vault();
    write(&paths, &config(json!([])));
    std::fs::set_permissions(paths.hooks_config(), std::fs::Permissions::from_mode(0o666)).unwrap();
    assert_eq!(
        HookSnapshot::load(&paths, &[]).await.unwrap_err(),
        HookConfigError::UntrustedFile
    );
    std::fs::remove_file(paths.hooks_config()).unwrap();
    let project = paths.wards_dir().join("project");
    std::fs::create_dir(&project).unwrap();
    let target = project.join("hooks.json");
    std::fs::write(&target, config(json!([])).to_string()).unwrap();
    symlink(&target, paths.hooks_config()).unwrap();
    assert_eq!(
        HookSnapshot::load(&paths, &[paths.wards_dir()])
            .await
            .unwrap_err(),
        HookConfigError::ProjectFile
    );
}

#[tokio::test]
async fn schema_numeric_integer_and_unicode_limits_are_preserved() {
    let (_temp, paths) = vault();
    let mut h = hook();
    h["timeout_ms"] = json!(5000.0);
    h["command"] = json!(["é".repeat(4096)]);
    write(&paths, &json!({"version":1.0,"hooks":[h]}));
    assert!(HookSnapshot::load(&paths, &[]).await.is_ok());
}
