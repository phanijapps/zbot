//! Map `surrealdb::Error` into `zero_stores::StoreError`.

use zero_stores::error::StoreError;

pub fn map_surreal_error(e: surrealdb::Error) -> StoreError {
    StoreError::Backend(format!("surrealdb: {e}"))
}

pub trait MapSurreal<T> {
    fn map_surreal(self) -> Result<T, StoreError>;
}

impl<T> MapSurreal<T> for Result<T, surrealdb::Error> {
    fn map_surreal(self) -> Result<T, StoreError> {
        self.map_err(map_surreal_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_to_backend_variant() {
        // SurrealDB 3.0 exposes `Error` as a struct with constructor methods
        // (no `Db::Thrown` enum variant). Use `Error::thrown(...)` to build any
        // variant; the test only verifies the mapping shape.
        let e = surrealdb::Error::thrown("boom".to_string());
        let mapped = map_surreal_error(e);
        match mapped {
            StoreError::Backend(s) => assert!(s.contains("surrealdb"), "got {s}"),
            other => panic!("expected Backend, got {other:?}"),
        }
    }
}
