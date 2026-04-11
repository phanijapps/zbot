//! Integration test: multimodal message round-trip.
//! Create → flush to disk → store (text only) → rehydrate → encode for API.

use base64::Engine;
use zero_core::multimodal::flush_parts_to_disk;
use zero_core::types::{ContentSource, ImageDetail, Part};
use zero_llm::encoding::ProviderEncoder;
use zero_llm::openai_encoder::{EncoderCapabilities, OpenAiEncoder};

#[test]
fn test_full_multimodal_roundtrip() {
    let dir = tempfile::tempdir().unwrap();

    // 1. Create multimodal parts (as an agent would)
    let original_data =
        base64::engine::general_purpose::STANDARD.encode(b"fake image bytes for testing");
    let parts = vec![
        Part::Text {
            text: "What do you see?".to_string(),
        },
        Part::Image {
            source: ContentSource::Base64(original_data.clone()),
            mime_type: "image/png".to_string(),
            detail: Some(ImageDetail::High),
        },
    ];

    // 2. Flush to disk (before DB persistence)
    let flushed = flush_parts_to_disk(parts, dir.path()).unwrap();
    assert!(matches!(&flushed[0], Part::Text { .. }));
    assert!(matches!(
        &flushed[1],
        Part::Image {
            source: ContentSource::FileRef(_),
            ..
        }
    ));

    // 3. Verify no Base64 in flushed parts (safe for DB)
    for part in &flushed {
        match part {
            Part::Image {
                source: ContentSource::Base64(_),
                ..
            } => panic!("Base64 should be flushed"),
            Part::File {
                source: ContentSource::Base64(_),
                ..
            } => panic!("Base64 should be flushed"),
            _ => {}
        }
    }

    // 4. Encode for API (rehydration happens inside encoder)
    let encoder = OpenAiEncoder::new(
        EncoderCapabilities {
            vision: true,
            tools: true,
        },
        "gpt-4o".to_string(),
    );
    let encoded = encoder.encode_content(&flushed).unwrap();

    // 5. Verify encoded format
    let arr = encoded.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["type"], "text");
    assert_eq!(arr[0]["text"], "What do you see?");
    assert_eq!(arr[1]["type"], "image_url");
    let url = arr[1]["image_url"]["url"].as_str().unwrap();
    assert!(url.starts_with("data:image/png;base64,"));
    assert_eq!(arr[1]["image_url"]["detail"], "high");

    // 6. Verify the base64 data survived the roundtrip
    let data_prefix = "data:image/png;base64,";
    let roundtripped_b64 = &url[data_prefix.len()..];
    assert_eq!(roundtripped_b64, original_data);
}

#[test]
fn test_text_only_backward_compat() {
    let encoder = OpenAiEncoder::new(
        EncoderCapabilities {
            vision: true,
            tools: true,
        },
        "gpt-4o".to_string(),
    );
    let parts = vec![Part::Text {
        text: "Just text".to_string(),
    }];
    let encoded = encoder.encode_content(&parts).unwrap();
    assert_eq!(encoded, serde_json::Value::String("Just text".to_string()));
}

#[test]
fn test_text_only_model_rejects_multimodal() {
    let encoder = OpenAiEncoder::new(
        EncoderCapabilities {
            vision: false,
            tools: true,
        },
        "gpt-3.5-turbo".to_string(),
    );
    let parts = vec![
        Part::Text {
            text: "Describe this".to_string(),
        },
        Part::Image {
            source: ContentSource::Base64("x".to_string()),
            mime_type: "image/png".to_string(),
            detail: None,
        },
    ];
    let err = encoder.encode_content(&parts).unwrap_err();
    assert!(err.to_string().contains("image"));
    assert!(err.to_string().contains("gpt-3.5-turbo"));
}
