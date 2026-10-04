//! Inert operator starter files. Never overwrite existing files or follow links.
use agent_primitives::vault_paths::VaultPaths;
use std::{fs::OpenOptions, io::Write};

pub(super) fn seed(paths: &VaultPaths) -> std::io::Result<()> {
    let vault = paths.vault_dir().canonicalize()?;
    let directory = paths.config_dir();
    if std::fs::symlink_metadata(&directory)?
        .file_type()
        .is_symlink()
        || directory.canonicalize()? != vault.join("config")
    {
        return Err(std::io::Error::other(
            "Hook config directory is unavailable",
        ));
    }
    for (path, content) in [
        (
            paths.hooks_schema(),
            include_str!("../../../contracts/jsonschema/hooks.schema.json"),
        ),
        (
            paths.hooks_config(),
            "{\n  \"$schema\": \"./hooks.schema.json\",\n  \"version\": 1,\n  \"hooks\": []\n}\n",
        ),
    ] {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                file.write_all(content.as_bytes())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn starter_is_inert_schema_exact_and_operator_edits_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().to_owned());
        paths.ensure_dirs_exist().unwrap();
        seed(&paths).unwrap();
        let config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(paths.hooks_config()).unwrap()).unwrap();
        assert_eq!(config["hooks"], serde_json::json!([]));
        assert_eq!(
            std::fs::read_to_string(paths.hooks_schema()).unwrap(),
            include_str!("../../../contracts/jsonschema/hooks.schema.json")
        );
        std::fs::write(paths.hooks_config(), "operator edit").unwrap();
        seed(&paths).unwrap();
        assert_eq!(
            std::fs::read_to_string(paths.hooks_config()).unwrap(),
            "operator edit"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(paths.hooks_config())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn starter_never_writes_through_file_or_directory_symlinks() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().to_owned());
        paths.ensure_dirs_exist().unwrap();
        let target = dir.path().join("operator-file");
        std::fs::write(&target, "preserve").unwrap();
        symlink(&target, paths.hooks_config()).unwrap();
        seed(&paths).unwrap();
        assert_eq!(std::fs::read_to_string(target).unwrap(), "preserve");
        std::fs::remove_dir_all(paths.config_dir()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), paths.config_dir()).unwrap();
        assert!(seed(&paths).is_err());
        assert!(!outside.path().join("hooks.json").exists());
    }
}
