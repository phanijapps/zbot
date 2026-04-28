//! Single SDK construction site. The `connect` function is the only place
//! in the codebase that interprets SurrealDB URL strings.

use std::path::Path;
use std::sync::Arc;

use surrealdb::engine::any::{connect as sdk_connect, Any};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

use crate::config::SurrealConfig;
use crate::error::map_surreal_error;
use zero_stores::error::StoreError;

/// Connect to a SurrealDB instance described by `cfg`.
///
/// `vault_root` is used to expand the `$VAULT` placeholder in the URL.
/// Pass `None` for tests using `mem://` URLs (no expansion needed).
pub async fn connect(
    cfg: &SurrealConfig,
    vault_root: Option<&Path>,
) -> Result<Arc<Surreal<Any>>, StoreError> {
    let url = expand_vault_placeholder(&cfg.url, vault_root)?;
    let db = sdk_connect(&url).await.map_err(map_surreal_error)?;

    if let Some(creds) = &cfg.credentials {
        db.signin(Root {
            username: creds.username.clone(),
            password: creds.password.clone(),
        })
        .await
        .map_err(map_surreal_error)?;
    }

    db.use_ns(&cfg.namespace)
        .use_db(&cfg.database)
        .await
        .map_err(map_surreal_error)?;

    Ok(Arc::new(db))
}

fn expand_vault_placeholder(url: &str, vault_root: Option<&Path>) -> Result<String, StoreError> {
    if !url.contains("$VAULT") {
        return Ok(url.to_string());
    }
    let root = vault_root.ok_or_else(|| {
        StoreError::Config("$VAULT placeholder used but no vault root provided".into())
    })?;
    let root_str = root
        .to_str()
        .ok_or_else(|| StoreError::Config("vault root path is not valid UTF-8".into()))?;
    Ok(url.replace("$VAULT", root_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_placeholder_no_op_when_absent() {
        let out = expand_vault_placeholder("mem://", None).unwrap();
        assert_eq!(out, "mem://");
    }

    #[test]
    fn vault_placeholder_errors_when_root_missing() {
        let result = expand_vault_placeholder("rocksdb://$VAULT/data", None);
        assert!(matches!(result, Err(StoreError::Config(_))));
    }

    #[test]
    fn vault_placeholder_substitutes() {
        let p = Path::new("/tmp/vault");
        let out = expand_vault_placeholder("rocksdb://$VAULT/x", Some(p)).unwrap();
        assert_eq!(out, "rocksdb:///tmp/vault/x");
    }
}
