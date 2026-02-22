// ═══════════════════════════════════════════════════════════════════
// Error Tests — CoreError variants, Display formatting, From impls
// ═══════════════════════════════════════════════════════════════════

use savings_tracker_core::errors::CoreError;

// ── Display formatting ──────────────────────────────────────────────

mod display {
    use super::*;

    #[test]
    fn invalid_file_format() {
        let err = CoreError::InvalidFileFormat("bad header".into());
        assert_eq!(err.to_string(), "Invalid file format: bad header");
    }

    #[test]
    fn invalid_file_format_empty_message() {
        let err = CoreError::InvalidFileFormat(String::new());
        assert_eq!(err.to_string(), "Invalid file format: ");
    }

    #[test]
    fn unsupported_version() {
        let err = CoreError::UnsupportedVersion(99);
        assert_eq!(err.to_string(), "Unsupported file version: 99");
    }

    #[test]
    fn unsupported_version_zero() {
        let err = CoreError::UnsupportedVersion(0);
        assert_eq!(err.to_string(), "Unsupported file version: 0");
    }

    #[test]
    fn unsupported_version_max() {
        let err = CoreError::UnsupportedVersion(u16::MAX);
        assert_eq!(
            err.to_string(),
            format!("Unsupported file version: {}", u16::MAX)
        );
    }

    #[test]
    fn encryption() {
        let err = CoreError::Encryption("AES key size invalid".into());
        assert_eq!(err.to_string(), "Encryption failed: AES key size invalid");
    }

    #[test]
    fn decryption() {
        let err = CoreError::Decryption;
        assert_eq!(
            err.to_string(),
            "Decryption failed — wrong password or corrupted file"
        );
    }

    #[test]
    fn serialization() {
        let err = CoreError::Serialization("buffer overflow".into());
        assert_eq!(err.to_string(), "Serialization error: buffer overflow");
    }

    #[test]
    fn deserialization() {
        let err = CoreError::Deserialization("unexpected EOF".into());
        assert_eq!(err.to_string(), "Deserialization error: unexpected EOF");
    }

    #[test]
    fn file_io() {
        let err = CoreError::FileIO("permission denied".into());
        assert_eq!(err.to_string(), "File I/O error: permission denied");
    }

    #[test]
    fn api_error() {
        let err = CoreError::Api {
            provider: "CoinCap".into(),
            message: "rate limited".into(),
        };
        assert_eq!(err.to_string(), "API error (CoinCap): rate limited");
    }

    #[test]
    fn api_error_empty_provider() {
        let err = CoreError::Api {
            provider: String::new(),
            message: "unknown".into(),
        };
        assert_eq!(err.to_string(), "API error (): unknown");
    }

    #[test]
    fn network() {
        let err = CoreError::Network("connection refused".into());
        assert_eq!(err.to_string(), "Network error: connection refused");
    }

    #[test]
    fn no_provider() {
        let err = CoreError::NoProvider("Crypto".into());
        assert_eq!(
            err.to_string(),
            "No provider available for asset type: Crypto"
        );
    }

    #[test]
    fn validation_error() {
        let err = CoreError::ValidationError("amount must be positive".into());
        assert_eq!(
            err.to_string(),
            "Event validation failed: amount must be positive"
        );
    }

    #[test]
    fn event_not_found() {
        let err = CoreError::EventNotFound("abc-123".into());
        assert_eq!(err.to_string(), "Event not found: abc-123");
    }

    #[test]
    fn price_not_available() {
        let err = CoreError::PriceNotAvailable {
            symbol: "BTC".into(),
            currency: "USD".into(),
            date: "2025-01-15".into(),
        };
        assert_eq!(
            err.to_string(),
            "Price not available for BTC in USD on 2025-01-15"
        );
    }

    #[test]
    fn price_not_available_empty_fields() {
        let err = CoreError::PriceNotAvailable {
            symbol: String::new(),
            currency: String::new(),
            date: String::new(),
        };
        assert_eq!(err.to_string(), "Price not available for  in  on ");
    }
}

// ── Debug trait ─────────────────────────────────────────────────────

mod debug_trait {
    use super::*;

    #[test]
    fn all_variants_are_debug() {
        // Ensure Debug is derived and doesn't panic
        let variants: Vec<CoreError> = vec![
            CoreError::InvalidFileFormat("test".into()),
            CoreError::UnsupportedVersion(1),
            CoreError::Encryption("test".into()),
            CoreError::Decryption,
            CoreError::Serialization("test".into()),
            CoreError::Deserialization("test".into()),
            CoreError::FileIO("test".into()),
            CoreError::Api {
                provider: "p".into(),
                message: "m".into(),
            },
            CoreError::Network("test".into()),
            CoreError::NoProvider("test".into()),
            CoreError::ValidationError("test".into()),
            CoreError::EventNotFound("test".into()),
            CoreError::PriceNotAvailable {
                symbol: "X".into(),
                currency: "Y".into(),
                date: "Z".into(),
            },
        ];

        for variant in &variants {
            let debug = format!("{:?}", variant);
            assert!(!debug.is_empty());
        }
    }
}

// ── From impls ──────────────────────────────────────────────────────

mod from_impls {
    use super::*;

    #[test]
    fn from_io_error_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let core_err: CoreError = io_err.into();
        match &core_err {
            CoreError::FileIO(msg) => assert!(msg.contains("file not found")),
            other => panic!("Expected FileIO, got {:?}", other),
        }
    }

    #[test]
    fn from_io_error_permission_denied() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let core_err: CoreError = io_err.into();
        match &core_err {
            CoreError::FileIO(msg) => assert!(msg.contains("access denied")),
            other => panic!("Expected FileIO, got {:?}", other),
        }
    }

    #[test]
    fn from_io_error_preserves_message() {
        let msg = "custom IO error with special chars: ąść";
        let io_err = std::io::Error::other(msg);
        let core_err: CoreError = io_err.into();
        match &core_err {
            CoreError::FileIO(m) => assert!(m.contains(msg)),
            other => panic!("Expected FileIO, got {:?}", other),
        }
    }

    #[test]
    fn from_bincode_error() {
        // Trigger a real bincode deserialization error
        let bad_data: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF];
        let result: Result<String, _> = bincode::deserialize(bad_data);
        let bincode_err = result.unwrap_err();
        let core_err: CoreError = bincode_err.into();
        match &core_err {
            CoreError::Serialization(msg) => assert!(!msg.is_empty()),
            other => panic!("Expected Serialization, got {:?}", other),
        }
    }

    #[test]
    fn from_serde_json_error() {
        // Trigger a real serde_json error
        let result: Result<String, _> = serde_json::from_str("{{invalid json");
        let json_err = result.unwrap_err();
        let core_err: CoreError = json_err.into();
        match &core_err {
            CoreError::Deserialization(msg) => {
                assert!(!msg.is_empty());
                // serde_json errors include line/column info
            }
            other => panic!("Expected Deserialization, got {:?}", other),
        }
    }

    #[test]
    fn from_serde_json_error_eof() {
        let result: Result<serde_json::Value, _> = serde_json::from_str("");
        let json_err = result.unwrap_err();
        let core_err: CoreError = json_err.into();
        match &core_err {
            CoreError::Deserialization(msg) => assert!(msg.contains("EOF")),
            other => panic!("Expected Deserialization, got {:?}", other),
        }
    }

    #[test]
    fn from_aes_gcm_error_via_decrypt() {
        // aes_gcm::Error is opaque; trigger it via decrypt with wrong key
        use savings_tracker_core::storage::encryption;

        let plaintext = b"hello world";
        let key = [1u8; 32];
        let nonce = [2u8; 12];
        let ciphertext = encryption::encrypt(plaintext, &key, &nonce).unwrap();

        // Decrypt with wrong key → aes_gcm::Error → CoreError::Decryption
        let wrong_key = [9u8; 32];
        let result = encryption::decrypt(&ciphertext, &wrong_key, &nonce);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::Decryption => {} // expected
            other => panic!("Expected Decryption, got {:?}", other),
        }
    }

    #[test]
    fn from_aes_gcm_error_via_wrong_nonce() {
        use savings_tracker_core::storage::encryption;

        let plaintext = b"secret data";
        let key = [3u8; 32];
        let nonce = [4u8; 12];
        let ciphertext = encryption::encrypt(plaintext, &key, &nonce).unwrap();

        // Decrypt with wrong nonce
        let wrong_nonce = [99u8; 12];
        let result = encryption::decrypt(&ciphertext, &key, &wrong_nonce);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::Decryption => {}
            other => panic!("Expected Decryption, got {:?}", other),
        }
    }
}

// ── Error is std::error::Error ──────────────────────────────────────

mod std_error {
    use super::*;

    #[test]
    fn core_error_implements_error_trait() {
        let err: Box<dyn std::error::Error> =
            Box::new(CoreError::InvalidFileFormat("test".into()));
        // Should compile and Display should work
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn core_error_implements_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CoreError>();
    }

    #[test]
    fn core_error_implements_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CoreError>();
    }
}

// ── Edge cases ──────────────────────────────────────────────────────

mod edge_cases {
    use super::*;

    #[test]
    fn very_long_error_message() {
        let long_msg = "x".repeat(10_000);
        let err = CoreError::Encryption(long_msg.clone());
        assert_eq!(err.to_string(), format!("Encryption failed: {}", long_msg));
    }

    #[test]
    fn unicode_in_error_message() {
        let err = CoreError::Api {
            provider: "日本API".into(),
            message: "接続エラー".into(),
        };
        assert_eq!(err.to_string(), "API error (日本API): 接続エラー");
    }

    #[test]
    fn newlines_in_error_message() {
        let err = CoreError::FileIO("line1\nline2\nline3".into());
        let display = err.to_string();
        assert!(display.contains("line1\nline2\nline3"));
    }

    #[test]
    fn price_not_available_with_special_chars() {
        let err = CoreError::PriceNotAvailable {
            symbol: "BTC/USD".into(),
            currency: "EUR€".into(),
            date: "2025-01-15T12:00:00".into(),
        };
        let display = err.to_string();
        assert!(display.contains("BTC/USD"));
        assert!(display.contains("EUR€"));
    }
}
