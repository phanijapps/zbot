//! Provider encoding trait — translates framework Part types to provider-specific wire format.

use zero_core::types::Part;

#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error("Unsupported content type '{part_type}' for model '{model}'")]
    UnsupportedContentType { part_type: String, model: String },

    #[error("Encoding failed: {reason}")]
    EncodingFailed { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub trait ProviderEncoder {
    fn encode_content(&self, parts: &[Part]) -> Result<serde_json::Value, EncodingError>;
    fn supports_part(&self, part: &Part) -> bool;
    fn filter_unsupported<'a>(&self, parts: &'a [Part]) -> (Vec<&'a Part>, Vec<&'a Part>) {
        let mut supported = vec![];
        let mut unsupported = vec![];
        for part in parts {
            if self.supports_part(part) {
                supported.push(part);
            } else {
                unsupported.push(part);
            }
        }
        (supported, unsupported)
    }
}
