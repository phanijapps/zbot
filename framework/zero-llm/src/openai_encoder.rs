//! OpenAI-compatible encoder — encodes Part types to OpenAI API wire format.

use serde_json::{json, Value};
use zero_core::multimodal::rehydrate_source;
use zero_core::types::{ContentSource, ImageDetail, Part};

use crate::encoding::{EncodingError, ProviderEncoder};

/// Describes what a model supports.
#[derive(Debug, Clone)]
pub struct EncoderCapabilities {
    pub vision: bool,
    pub tools: bool,
}

/// Encodes `Part` slices into OpenAI-compatible JSON content.
pub struct OpenAiEncoder {
    capabilities: EncoderCapabilities,
    model_id: String,
}

impl OpenAiEncoder {
    pub fn new(capabilities: EncoderCapabilities, model_id: String) -> Self {
        Self {
            capabilities,
            model_id,
        }
    }
}

impl ProviderEncoder for OpenAiEncoder {
    fn supports_part(&self, part: &Part) -> bool {
        match part {
            Part::Text { .. } => true,
            Part::Image { .. } => self.capabilities.vision,
            Part::File { .. } => self.capabilities.vision,
            Part::FunctionCall { .. } | Part::FunctionResponse { .. } => self.capabilities.tools,
        }
    }

    fn encode_content(&self, parts: &[Part]) -> Result<Value, EncodingError> {
        // Validate all parts
        for part in parts {
            if !self.supports_part(part) {
                return Err(EncodingError::UnsupportedContentType {
                    part_type: part.type_name().to_string(),
                    model: self.model_id.clone(),
                });
            }
        }

        // Backward compat: text-only → plain string
        let has_multimodal = parts.iter().any(|p| p.is_multimodal());
        if !has_multimodal {
            let text: String = parts
                .iter()
                .filter_map(|p| match p {
                    Part::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(Value::String(text));
        }

        // Multimodal: encode as content array
        let mut blocks = Vec::new();
        for part in parts {
            blocks.push(encode_part(part)?);
        }
        Ok(Value::Array(blocks))
    }
}

fn encode_part(part: &Part) -> Result<Value, EncodingError> {
    match part {
        Part::Text { text } => Ok(json!({ "type": "text", "text": text })),
        Part::Image {
            source,
            mime_type,
            detail,
        } => {
            let resolved = rehydrate_source(source)?;
            let url = match &resolved {
                ContentSource::Base64(data) => format!("data:{};base64,{}", mime_type, data),
                ContentSource::Url(url) => url.clone(),
                ContentSource::FileRef(_) => unreachable!("rehydrate always resolves FileRef"),
            };
            let mut image_url = json!({ "url": url });
            if let Some(d) = detail {
                let detail_str = match d {
                    ImageDetail::Low => "low",
                    ImageDetail::High => "high",
                    ImageDetail::Auto => "auto",
                };
                image_url
                    .as_object_mut()
                    .unwrap()
                    .insert("detail".to_string(), json!(detail_str));
            }
            Ok(json!({ "type": "image_url", "image_url": image_url }))
        }
        Part::File {
            source, mime_type, ..
        } => {
            let resolved = rehydrate_source(source)?;
            let url = match &resolved {
                ContentSource::Base64(data) => format!("data:{};base64,{}", mime_type, data),
                ContentSource::Url(url) => url.clone(),
                ContentSource::FileRef(_) => unreachable!("rehydrate always resolves FileRef"),
            };
            Ok(json!({ "type": "file", "file": { "url": url } }))
        }
        Part::FunctionCall { .. } | Part::FunctionResponse { .. } => {
            Err(EncodingError::EncodingFailed {
                reason: "FunctionCall/FunctionResponse should not be in content blocks".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::types::ContentSource;

    fn vision_encoder() -> OpenAiEncoder {
        OpenAiEncoder::new(
            EncoderCapabilities {
                vision: true,
                tools: true,
            },
            "gpt-4o".to_string(),
        )
    }

    fn text_only_encoder() -> OpenAiEncoder {
        OpenAiEncoder::new(
            EncoderCapabilities {
                vision: false,
                tools: true,
            },
            "gpt-3.5-turbo".to_string(),
        )
    }

    #[test]
    fn test_text_only_returns_string() {
        let encoder = vision_encoder();
        let parts = vec![Part::text("hello world")];
        let result = encoder.encode_content(&parts).unwrap();
        assert_eq!(result, Value::String("hello world".to_string()));
    }

    #[test]
    fn test_text_only_multiple_parts_joined() {
        let encoder = vision_encoder();
        let parts = vec![Part::text("line 1"), Part::text("line 2")];
        let result = encoder.encode_content(&parts).unwrap();
        assert_eq!(result, Value::String("line 1\nline 2".to_string()));
    }

    #[test]
    fn test_image_base64_encodes_as_data_uri() {
        let encoder = vision_encoder();
        let parts = vec![
            Part::text("What is in this image?"),
            Part::Image {
                source: ContentSource::Base64("aGVsbG8=".to_string()),
                mime_type: "image/png".to_string(),
                detail: Some(ImageDetail::High),
            },
        ];
        let result = encoder.encode_content(&parts).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[0]["text"], "What is in this image?");
        assert_eq!(arr[1]["type"], "image_url");
        assert_eq!(
            arr[1]["image_url"]["url"],
            "data:image/png;base64,aGVsbG8="
        );
        assert_eq!(arr[1]["image_url"]["detail"], "high");
    }

    #[test]
    fn test_image_url_passes_through() {
        let encoder = vision_encoder();
        let parts = vec![Part::Image {
            source: ContentSource::Url("https://example.com/img.png".to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        }];
        let result = encoder.encode_content(&parts).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(
            arr[0]["image_url"]["url"],
            "https://example.com/img.png"
        );
        assert!(arr[0]["image_url"].get("detail").is_none());
    }

    #[test]
    fn test_file_encodes_correctly() {
        let encoder = vision_encoder();
        let parts = vec![Part::File {
            source: ContentSource::Base64("cGRmZGF0YQ==".to_string()),
            mime_type: "application/pdf".to_string(),
            filename: Some("report.pdf".to_string()),
        }];
        let result = encoder.encode_content(&parts).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0]["type"], "file");
        assert_eq!(
            arr[0]["file"]["url"],
            "data:application/pdf;base64,cGRmZGF0YQ=="
        );
    }

    #[test]
    fn test_text_only_model_rejects_image() {
        let encoder = text_only_encoder();
        let parts = vec![Part::Image {
            source: ContentSource::Base64("x".to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        }];
        let err = encoder.encode_content(&parts).unwrap_err();
        match err {
            EncodingError::UnsupportedContentType { part_type, model } => {
                assert_eq!(part_type, "image");
                assert_eq!(model, "gpt-3.5-turbo");
            }
            _ => panic!("Expected UnsupportedContentType"),
        }
    }

    #[test]
    fn test_text_only_model_accepts_text() {
        let encoder = text_only_encoder();
        let parts = vec![Part::text("hello")];
        let result = encoder.encode_content(&parts).unwrap();
        assert_eq!(result, Value::String("hello".to_string()));
    }

    #[test]
    fn test_supports_part() {
        let vision = vision_encoder();
        let text_only = text_only_encoder();
        let text = Part::text("hi");
        let image = Part::Image {
            source: ContentSource::Base64("x".to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        };
        assert!(vision.supports_part(&text));
        assert!(vision.supports_part(&image));
        assert!(text_only.supports_part(&text));
        assert!(!text_only.supports_part(&image));
    }

    #[test]
    fn test_filter_unsupported() {
        let encoder = text_only_encoder();
        let parts = vec![
            Part::text("hello"),
            Part::Image {
                source: ContentSource::Base64("x".to_string()),
                mime_type: "image/png".to_string(),
                detail: None,
            },
        ];
        let (supported, unsupported) = encoder.filter_unsupported(&parts);
        assert_eq!(supported.len(), 1);
        assert_eq!(unsupported.len(), 1);
    }

    #[test]
    fn test_image_fileref_rehydrated() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.png");
        std::fs::write(&file_path, b"fake png").unwrap();
        let encoder = vision_encoder();
        let parts = vec![Part::Image {
            source: ContentSource::FileRef(file_path.to_str().unwrap().to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        }];
        let result = encoder.encode_content(&parts).unwrap();
        let arr = result.as_array().unwrap();
        let url = arr[0]["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
    }
}
