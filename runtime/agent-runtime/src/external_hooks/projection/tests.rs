use super::*;
use serde_json::json;
#[test]
fn recursive_credentials_containers_and_registered_values_never_escape() {
    let value = json!({"Auth_orization":"credential","nested":{"api-key":"private","query":"safe registered-value suffix","rows":[{"OAuthToken":"bad","name":"ok"}]},"headers":[{"name":"Authorization","value":"hidden"}],"environment":{"VALUE":"hidden"},"safe":true});
    let result = project_arguments(&value, &["registered-value".into()]);
    assert!(result.redacted);
    assert_eq!(
        result.arguments,
        json!({"nested":{"query":"safe [redacted] suffix","rows":[{"name":"ok"}]},"safe":true})
    );
    let safe = json!({"query":"public","array":[1,true,"normal"]});
    let result = project_arguments(&safe, &[]);
    assert_eq!(result.arguments, safe);
    assert!(!result.redacted);
}
#[test]
fn invalid_or_unprojectable_arguments_are_empty_and_flagged() {
    for value in [json!([{"query":"safe"}]), json!("secret"), json!(null)] {
        let result = project_arguments(&value, &[]);
        assert_eq!(result.arguments, json!({}));
        assert!(result.redacted);
    }
    let mut deep = json!({"payload":"sentinel"});
    for _ in 0..80 {
        deep = json!({"child":deep});
    }
    let result = project_arguments(&deep, &[]);
    assert_eq!(result.arguments, json!({}));
    assert!(result.redacted);
}
