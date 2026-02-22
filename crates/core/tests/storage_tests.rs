// ═══════════════════════════════════════════════════════════════════
// Storage Tests — encryption, file format, StorageManager
// ═══════════════════════════════════════════════════════════════════

use chrono::NaiveDate;
use savings_tracker_core::errors::CoreError;
use savings_tracker_core::models::asset::Asset;
use savings_tracker_core::models::event::{Event, EventType};
use savings_tracker_core::models::portfolio::Portfolio;
use savings_tracker_core::storage::encryption::{
    derive_key, decrypt, encrypt, generate_nonce, generate_salt, KdfParams,
};
use savings_tracker_core::storage::format::{self, CURRENT_VERSION, MAGIC, MIN_HEADER_SIZE};
use savings_tracker_core::storage::manager::StorageManager;

// ═══════════════════════════════════════════════════════════════════
// KdfParams
// ═══════════════════════════════════════════════════════════════════

mod kdf_params {
    use super::*;

    #[test]
    fn default_values() {
        let p = KdfParams::default();
        assert_eq!(p.memory_cost, 65_536);
        assert_eq!(p.time_cost, 3);
        assert_eq!(p.parallelism, 4);
    }

    #[test]
    fn custom_values() {
        let p = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        assert_eq!(p.memory_cost, 1024);
        assert_eq!(p.time_cost, 1);
        assert_eq!(p.parallelism, 1);
    }

    #[test]
    fn clone_and_copy() {
        let p = KdfParams::default();
        let p2 = p;
        let p3 = p;
        assert_eq!(p2.memory_cost, p3.memory_cost);
        assert_eq!(p2.time_cost, p3.time_cost);
        assert_eq!(p2.parallelism, p3.parallelism);
    }

    #[test]
    fn debug_format() {
        let p = KdfParams::default();
        let debug = format!("{:?}", p);
        assert!(debug.contains("65536"));
        assert!(debug.contains("KdfParams"));
    }
}

// ═══════════════════════════════════════════════════════════════════
// Key Derivation
// ═══════════════════════════════════════════════════════════════════

mod key_derivation {
    use super::*;

    #[test]
    fn derive_key_produces_32_bytes() {
        let salt = [1u8; 16];
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        let key = derive_key("password", &salt, &params).unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn derive_key_deterministic() {
        let salt = [42u8; 16];
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        let key1 = derive_key("same-password", &salt, &params).unwrap();
        let key2 = derive_key("same-password", &salt, &params).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn derive_key_different_passwords_different_keys() {
        let salt = [7u8; 16];
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        let key1 = derive_key("password-a", &salt, &params).unwrap();
        let key2 = derive_key("password-b", &salt, &params).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_key_different_salts_different_keys() {
        let salt1 = [1u8; 16];
        let salt2 = [2u8; 16];
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        let key1 = derive_key("same-password", &salt1, &params).unwrap();
        let key2 = derive_key("same-password", &salt2, &params).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_key_empty_password() {
        let salt = [0u8; 16];
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        let result = derive_key("", &salt, &params);
        // Empty password should still work (user may choose to have no password)
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }

    #[test]
    fn derive_key_unicode_password() {
        let salt = [5u8; 16];
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        let result = derive_key("пароль日本語🔑", &salt, &params);
        assert!(result.is_ok());
    }

    #[test]
    fn derive_key_very_long_password() {
        let salt = [6u8; 16];
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };
        let long_pass = "a".repeat(10_000);
        let result = derive_key(&long_pass, &salt, &params);
        assert!(result.is_ok());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Encrypt / Decrypt
// ═══════════════════════════════════════════════════════════════════

mod encrypt_decrypt {
    use super::*;

    #[test]
    fn round_trip_basic() {
        let key = [42u8; 32];
        let nonce = [7u8; 12];
        let plaintext = b"hello, world!";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn round_trip_empty_plaintext() {
        let key = [1u8; 32];
        let nonce = [2u8; 12];
        let plaintext = b"";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        // Even empty plaintext produces ciphertext (auth tag)
        assert!(!ciphertext.is_empty());
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn round_trip_large_data() {
        let key = [3u8; 32];
        let nonce = [4u8; 12];
        let plaintext: Vec<u8> = (0u8..=255).cycle().take(100_000).collect();

        let ciphertext = encrypt(&plaintext, &key, &nonce).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn ciphertext_differs_from_plaintext() {
        let key = [5u8; 32];
        let nonce = [6u8; 12];
        let plaintext = b"sensitive data";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        assert_ne!(&ciphertext[..], &plaintext[..]);
    }

    #[test]
    fn ciphertext_larger_than_plaintext() {
        let key = [7u8; 32];
        let nonce = [8u8; 12];
        let plaintext = b"test";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        // AES-GCM adds a 16-byte auth tag
        assert_eq!(ciphertext.len(), plaintext.len() + 16);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key = [10u8; 32];
        let nonce = [11u8; 12];
        let plaintext = b"secret";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        let wrong_key = [99u8; 32];
        assert!(decrypt(&ciphertext, &wrong_key, &nonce).is_err());
    }

    #[test]
    fn decrypt_with_wrong_nonce_fails() {
        let key = [12u8; 32];
        let nonce = [13u8; 12];
        let plaintext = b"secret";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        let wrong_nonce = [99u8; 12];
        assert!(decrypt(&ciphertext, &key, &wrong_nonce).is_err());
    }

    #[test]
    fn decrypt_tampered_ciphertext_fails() {
        let key = [14u8; 32];
        let nonce = [15u8; 12];
        let plaintext = b"integrity check";

        let mut ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        // Flip a bit in the ciphertext
        if let Some(byte) = ciphertext.get_mut(0) {
            *byte ^= 0xFF;
        }
        assert!(decrypt(&ciphertext, &key, &nonce).is_err());
    }

    #[test]
    fn decrypt_truncated_ciphertext_fails() {
        let key = [16u8; 32];
        let nonce = [17u8; 12];
        let plaintext = b"truncation test";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        let truncated = &ciphertext[..ciphertext.len() - 1];
        assert!(decrypt(truncated, &key, &nonce).is_err());
    }

    #[test]
    fn decrypt_empty_ciphertext_fails() {
        let key = [18u8; 32];
        let nonce = [19u8; 12];
        assert!(decrypt(&[], &key, &nonce).is_err());
    }

    #[test]
    fn same_plaintext_same_key_same_nonce_same_ciphertext() {
        let key = [20u8; 32];
        let nonce = [21u8; 12];
        let plaintext = b"deterministic";

        let ct1 = encrypt(plaintext, &key, &nonce).unwrap();
        let ct2 = encrypt(plaintext, &key, &nonce).unwrap();
        assert_eq!(ct1, ct2);
    }

    #[test]
    fn same_plaintext_different_nonce_different_ciphertext() {
        let key = [22u8; 32];
        let nonce1 = [23u8; 12];
        let nonce2 = [24u8; 12];
        let plaintext = b"nonce matters";

        let ct1 = encrypt(plaintext, &key, &nonce1).unwrap();
        let ct2 = encrypt(plaintext, &key, &nonce2).unwrap();
        assert_ne!(ct1, ct2);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Random Generation
// ═══════════════════════════════════════════════════════════════════

mod random_generation {
    use super::*;

    #[test]
    fn generate_salt_returns_16_bytes() {
        let salt = generate_salt().unwrap();
        assert_eq!(salt.len(), 16);
    }

    #[test]
    fn generate_salt_unique() {
        let salt1 = generate_salt().unwrap();
        let salt2 = generate_salt().unwrap();
        assert_ne!(salt1, salt2);
    }

    #[test]
    fn generate_nonce_returns_12_bytes() {
        let nonce = generate_nonce().unwrap();
        assert_eq!(nonce.len(), 12);
    }

    #[test]
    fn generate_nonce_unique() {
        let nonce1 = generate_nonce().unwrap();
        let nonce2 = generate_nonce().unwrap();
        assert_ne!(nonce1, nonce2);
    }

    #[test]
    fn salts_not_all_zeroes() {
        // Extremely unlikely but worth checking generation works
        let salt = generate_salt().unwrap();
        assert!(salt.iter().any(|&b| b != 0));
    }

    #[test]
    fn nonces_not_all_zeroes() {
        let nonce = generate_nonce().unwrap();
        assert!(nonce.iter().any(|&b| b != 0));
    }
}

// ═══════════════════════════════════════════════════════════════════
// File Format — write_file / read_file
// ═══════════════════════════════════════════════════════════════════

mod file_format {
    use super::*;

    fn make_test_file(ciphertext: &[u8]) -> Vec<u8> {
        let kdf = KdfParams::default();
        let salt = [0xAA; 16];
        let nonce = [0xBB; 12];
        format::write_file(CURRENT_VERSION, &kdf, &salt, &nonce, ciphertext)
    }

    #[test]
    fn write_read_round_trip() {
        let ciphertext = b"encrypted-data-here";
        let file_bytes = make_test_file(ciphertext);

        let (header, ct) = format::read_file(&file_bytes).unwrap();
        assert_eq!(header.version, CURRENT_VERSION);
        assert_eq!(header.salt, [0xAA; 16]);
        assert_eq!(header.nonce, [0xBB; 12]);
        assert_eq!(header.ciphertext_len, ciphertext.len() as u64);
        assert_eq!(ct, ciphertext);
    }

    #[test]
    fn write_read_round_trip_empty_ciphertext() {
        let file_bytes = make_test_file(b"");

        let (header, ct) = format::read_file(&file_bytes).unwrap();
        assert_eq!(header.ciphertext_len, 0);
        assert_eq!(ct.len(), 0);
    }

    #[test]
    fn write_read_round_trip_large_ciphertext() {
        let ciphertext: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
        let kdf = KdfParams::default();
        let salt = [0x11; 16];
        let nonce = [0x22; 12];
        let file_bytes = format::write_file(CURRENT_VERSION, &kdf, &salt, &nonce, &ciphertext);

        let (header, ct) = format::read_file(&file_bytes).unwrap();
        assert_eq!(ct, &ciphertext[..]);
        assert_eq!(header.ciphertext_len, 10_000);
    }

    #[test]
    fn magic_bytes_at_start() {
        let file_bytes = make_test_file(b"test");
        assert_eq!(&file_bytes[0..4], MAGIC);
    }

    #[test]
    fn version_at_correct_offset() {
        let file_bytes = make_test_file(b"test");
        let version = u16::from_le_bytes([file_bytes[4], file_bytes[5]]);
        assert_eq!(version, CURRENT_VERSION);
    }

    #[test]
    fn kdf_params_at_correct_offset() {
        let kdf = KdfParams {
            memory_cost: 12345,
            time_cost: 67,
            parallelism: 8,
        };
        let salt = [0; 16];
        let nonce = [0; 12];
        let file_bytes = format::write_file(CURRENT_VERSION, &kdf, &salt, &nonce, b"ct");

        let mem = u32::from_le_bytes(file_bytes[6..10].try_into().unwrap());
        let time = u32::from_le_bytes(file_bytes[10..14].try_into().unwrap());
        let par = u32::from_le_bytes(file_bytes[14..18].try_into().unwrap());
        assert_eq!(mem, 12345);
        assert_eq!(time, 67);
        assert_eq!(par, 8);
    }

    #[test]
    fn header_kdf_params_preserved() {
        let kdf = KdfParams {
            memory_cost: 999,
            time_cost: 7,
            parallelism: 2,
        };
        let salt = [0xCC; 16];
        let nonce = [0xDD; 12];
        let file_bytes = format::write_file(CURRENT_VERSION, &kdf, &salt, &nonce, b"test");

        let (header, _) = format::read_file(&file_bytes).unwrap();
        assert_eq!(header.kdf_params.memory_cost, 999);
        assert_eq!(header.kdf_params.time_cost, 7);
        assert_eq!(header.kdf_params.parallelism, 2);
    }

    #[test]
    fn file_too_small() {
        let data = vec![0u8; MIN_HEADER_SIZE - 1];
        let result = format::read_file(&data);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::InvalidFileFormat(msg) => assert!(msg.contains("too small")),
            other => panic!("Expected InvalidFileFormat, got {:?}", other),
        }
    }

    #[test]
    fn file_empty() {
        let result = format::read_file(&[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::InvalidFileFormat(msg) => assert!(msg.contains("too small")),
            other => panic!("Expected InvalidFileFormat, got {:?}", other),
        }
    }

    #[test]
    fn wrong_magic_bytes() {
        let mut file_bytes = make_test_file(b"test");
        file_bytes[0] = b'X';
        file_bytes[1] = b'Y';
        file_bytes[2] = b'Z';
        file_bytes[3] = b'W';

        let result = format::read_file(&file_bytes);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::InvalidFileFormat(msg) => assert!(msg.contains("magic")),
            other => panic!("Expected InvalidFileFormat, got {:?}", other),
        }
    }

    #[test]
    fn unsupported_version() {
        let kdf = KdfParams::default();
        let salt = [0; 16];
        let nonce = [0; 12];
        // Write with a future version
        let file_bytes = format::write_file(CURRENT_VERSION + 1, &kdf, &salt, &nonce, b"test");

        let result = format::read_file(&file_bytes);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::UnsupportedVersion(v) => assert_eq!(v, CURRENT_VERSION + 1),
            other => panic!("Expected UnsupportedVersion, got {:?}", other),
        }
    }

    #[test]
    fn truncated_ciphertext() {
        let mut file_bytes = make_test_file(b"some ciphertext data here!!");
        // Chop off last 10 bytes of ciphertext
        file_bytes.truncate(file_bytes.len() - 10);

        let result = format::read_file(&file_bytes);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::InvalidFileFormat(msg) => assert!(msg.contains("truncated")),
            other => panic!("Expected InvalidFileFormat, got {:?}", other),
        }
    }

    #[test]
    fn extra_bytes_after_ciphertext_ignored() {
        let mut file_bytes = make_test_file(b"cipher");
        // Append extra garbage — should still read successfully
        file_bytes.extend_from_slice(b"garbage");

        let (header, ct) = format::read_file(&file_bytes).unwrap();
        assert_eq!(ct, b"cipher");
        assert_eq!(header.ciphertext_len, 6);
    }

    #[test]
    fn file_header_is_debug() {
        let file_bytes = make_test_file(b"test");
        let (header, _) = format::read_file(&file_bytes).unwrap();
        let debug = format!("{:?}", header);
        assert!(debug.contains("FileHeader"));
    }

    #[test]
    fn min_header_size_constant() {
        // 4 (magic) + 2 (version) + 12 (kdf) + 16 (salt) + 12 (nonce) + 8 (len) = 54
        assert_eq!(MIN_HEADER_SIZE, 54);
    }

    #[test]
    fn current_version_is_one() {
        assert_eq!(CURRENT_VERSION, 1);
    }

    #[test]
    fn magic_is_svtk() {
        assert_eq!(MAGIC, b"SVTK");
    }

    #[test]
    fn total_file_size_correct() {
        let ct = b"1234567890";
        let file_bytes = make_test_file(ct);
        assert_eq!(file_bytes.len(), MIN_HEADER_SIZE + ct.len());
    }
}

// ═══════════════════════════════════════════════════════════════════
// StorageManager
// ═══════════════════════════════════════════════════════════════════

mod storage_manager {
    use super::*;

    #[test]
    fn save_load_empty_portfolio() {
        let portfolio = Portfolio::default();
        let password = "test-password";

        let bytes = StorageManager::save_to_bytes(&portfolio, password).unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, password).unwrap();

        assert_eq!(loaded.events.len(), 0);
        assert_eq!(
            loaded.settings.default_currency,
            portfolio.settings.default_currency
        );
    }

    #[test]
    fn save_load_portfolio_with_events() {
        let mut portfolio = Portfolio::default();
        portfolio.settings.default_currency = "EUR".into();
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.5,
            NaiveDate::from_ymd_opt(2025, 6, 15).unwrap(),
        ));
        portfolio.events.push(Event::new(
            EventType::Sell,
            Asset::stock("AAPL", "Apple Inc."),
            10.0,
            NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
        ));

        let password = "events-password!";
        let bytes = StorageManager::save_to_bytes(&portfolio, password).unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, password).unwrap();

        assert_eq!(loaded.events.len(), 2);
        assert_eq!(loaded.settings.default_currency, "EUR");
        assert_eq!(loaded.events[0].asset.symbol, "BTC");
        assert_eq!(loaded.events[0].amount, 1.5);
        assert_eq!(loaded.events[1].asset.symbol, "AAPL");
    }

    #[test]
    fn save_load_portfolio_with_price_cache() {
        let mut portfolio = Portfolio::default();
        let date = NaiveDate::from_ymd_opt(2025, 3, 1).unwrap();
        portfolio.price_cache.set_price("ETH", "USD", date, 3000.0);
        portfolio
            .price_cache
            .set_price("XAU", "USD", date, 2050.0);

        let password = "cache-pw";
        let bytes = StorageManager::save_to_bytes(&portfolio, password).unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, password).unwrap();

        assert_eq!(loaded.price_cache.get_price("ETH", "USD", date), Some(3000.0));
        assert_eq!(loaded.price_cache.get_price("XAU", "USD", date), Some(2050.0));
    }

    #[test]
    fn save_load_portfolio_with_settings() {
        let mut portfolio = Portfolio::default();
        portfolio.settings.default_currency = "PLN".to_string();
        portfolio
            .settings
            .api_keys
            .insert("metals_dev".into(), "key123".into());
        portfolio
            .settings
            .api_keys
            .insert("alphavantage".into(), "av-key".into());

        let password = "settings-pw";
        let bytes = StorageManager::save_to_bytes(&portfolio, password).unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, password).unwrap();

        assert_eq!(loaded.settings.default_currency, "PLN");
        assert_eq!(
            loaded.settings.api_keys.get("metals_dev").unwrap(),
            "key123"
        );
        assert_eq!(
            loaded.settings.api_keys.get("alphavantage").unwrap(),
            "av-key"
        );
    }

    #[test]
    fn wrong_password_fails() {
        let portfolio = Portfolio::default();
        let bytes = StorageManager::save_to_bytes(&portfolio, "correct").unwrap();
        let result = StorageManager::load_from_bytes(&bytes, "wrong");
        assert!(result.is_err());
    }

    #[test]
    fn corrupted_data_fails() {
        let portfolio = Portfolio::default();
        let mut bytes = StorageManager::save_to_bytes(&portfolio, "pass").unwrap();

        // Corrupt a byte in the ciphertext area
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;

        let result = StorageManager::load_from_bytes(&bytes, "pass");
        assert!(result.is_err());
    }

    #[test]
    fn empty_data_fails() {
        let result = StorageManager::load_from_bytes(&[], "pass");
        assert!(result.is_err());
    }

    #[test]
    fn garbage_data_fails() {
        let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];
        let result = StorageManager::load_from_bytes(&garbage, "pass");
        assert!(result.is_err());
    }

    #[test]
    fn empty_password_round_trip() {
        let portfolio = Portfolio::default();
        let bytes = StorageManager::save_to_bytes(&portfolio, "").unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, "").unwrap();
        assert_eq!(loaded.events.len(), 0);
    }

    #[test]
    fn unicode_password_round_trip() {
        let portfolio = Portfolio::default();
        let password = "пароль🔑日本語";
        let bytes = StorageManager::save_to_bytes(&portfolio, password).unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, password).unwrap();
        assert_eq!(loaded.events.len(), 0);
    }

    #[test]
    fn long_password_round_trip() {
        let portfolio = Portfolio::default();
        let password = "x".repeat(1_000);
        let bytes = StorageManager::save_to_bytes(&portfolio, &password).unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, &password).unwrap();
        assert_eq!(loaded.events.len(), 0);
    }

    #[test]
    fn save_produces_different_bytes_each_time() {
        // Due to random salt/nonce, two saves should produce different ciphertext
        let portfolio = Portfolio::default();
        let bytes1 = StorageManager::save_to_bytes(&portfolio, "pass").unwrap();
        let bytes2 = StorageManager::save_to_bytes(&portfolio, "pass").unwrap();
        assert_ne!(bytes1, bytes2);
    }

    #[test]
    fn output_starts_with_magic() {
        let portfolio = Portfolio::default();
        let bytes = StorageManager::save_to_bytes(&portfolio, "pw").unwrap();
        assert_eq!(&bytes[0..4], b"SVTK");
    }

    #[test]
    fn output_has_current_version() {
        let portfolio = Portfolio::default();
        let bytes = StorageManager::save_to_bytes(&portfolio, "pw").unwrap();
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        assert_eq!(version, CURRENT_VERSION);
    }

    #[test]
    fn save_load_preserves_event_ids() {
        let mut portfolio = Portfolio::default();
        let event = Event::new(
            EventType::Buy,
            Asset::crypto("SOL", "Solana"),
            100.0,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        );
        let original_id = event.id;
        portfolio.events.push(event);

        let bytes = StorageManager::save_to_bytes(&portfolio, "pw").unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, "pw").unwrap();

        assert_eq!(loaded.events[0].id, original_id);
    }

    #[test]
    fn save_load_preserves_event_dates() {
        let mut portfolio = Portfolio::default();
        let date = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::fiat("EUR", "Euro"),
            500.0,
            date,
        ));

        let bytes = StorageManager::save_to_bytes(&portfolio, "pw").unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, "pw").unwrap();

        assert_eq!(loaded.events[0].date, date);
    }

    #[test]
    fn save_load_many_events() {
        let mut portfolio = Portfolio::default();
        for i in 0..100 {
            portfolio.events.push(Event::new(
                if i % 2 == 0 {
                    EventType::Buy
                } else {
                    EventType::Sell
                },
                Asset::crypto("BTC", "Bitcoin"),
                (i as f64) * 0.01 + 0.01,
                NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            ));
        }

        let bytes = StorageManager::save_to_bytes(&portfolio, "many").unwrap();
        let loaded = StorageManager::load_from_bytes(&bytes, "many").unwrap();

        assert_eq!(loaded.events.len(), 100);
    }
}

// ═══════════════════════════════════════════════════════════════════
// StorageManager — File I/O (native only)
// ═══════════════════════════════════════════════════════════════════

#[cfg(not(target_arch = "wasm32"))]
mod file_io {
    use super::*;

    #[test]
    fn save_and_load_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.svtk");
        let path_str = path.to_str().unwrap();

        let mut portfolio = Portfolio::default();
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("ETH", "Ethereum"),
            2.0,
            NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
        ));

        StorageManager::save_to_file(&portfolio, path_str, "file-pw").unwrap();
        let loaded = StorageManager::load_from_file(path_str, "file-pw").unwrap();

        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.events[0].asset.symbol, "ETH");
    }

    #[test]
    fn load_nonexistent_file_fails() {
        let result = StorageManager::load_from_file("/tmp/nonexistent_svtk_file.svtk", "pw");
        assert!(result.is_err());
    }

    #[test]
    fn overwrite_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overwrite.svtk");
        let path_str = path.to_str().unwrap();

        let portfolio1 = Portfolio::default();
        StorageManager::save_to_file(&portfolio1, path_str, "pw").unwrap();

        let mut portfolio2 = Portfolio::default();
        portfolio2.events.push(Event::new(
            EventType::Buy,
            Asset::metal("XAU", "Gold"),
            1.0,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        ));
        StorageManager::save_to_file(&portfolio2, path_str, "pw").unwrap();

        let loaded = StorageManager::load_from_file(path_str, "pw").unwrap();
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.events[0].asset.symbol, "XAU");
    }

    #[test]
    fn file_wrong_password_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wrong_pw.svtk");
        let path_str = path.to_str().unwrap();

        let portfolio = Portfolio::default();
        StorageManager::save_to_file(&portfolio, path_str, "correct").unwrap();
        let result = StorageManager::load_from_file(path_str, "incorrect");
        assert!(result.is_err());
    }

    #[test]
    fn file_has_svtk_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("magic.svtk");
        let path_str = path.to_str().unwrap();

        let portfolio = Portfolio::default();
        StorageManager::save_to_file(&portfolio, path_str, "pw").unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"SVTK");
    }
}

// ═══════════════════════════════════════════════════════════════════
// End-to-end encryption pipeline
// ═══════════════════════════════════════════════════════════════════

mod e2e_encryption {
    use super::*;

    #[test]
    fn full_pipeline_derive_encrypt_decrypt() {
        let password = "my-secure-password!";
        let salt = [0xABu8; 16];
        let nonce = [0xCDu8; 12];
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };

        let key = derive_key(password, &salt, &params).unwrap();
        let plaintext = b"portfolio data as bincode bytes";
        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();

        // Write to file format
        let file_bytes =
            format::write_file(CURRENT_VERSION, &params, &salt, &nonce, &ciphertext);

        // Read back
        let (header, ct) = format::read_file(&file_bytes).unwrap();

        // Re-derive key and decrypt
        let key2 = derive_key(password, &header.salt, &header.kdf_params).unwrap();
        assert_eq!(key, key2);

        let decrypted = decrypt(ct, &key2, &header.nonce).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn full_pipeline_wrong_password() {
        let password = "correct";
        let salt = [0x11u8; 16];
        let nonce = [0x22u8; 12];
        let params = KdfParams {
            memory_cost: 1024,
            time_cost: 1,
            parallelism: 1,
        };

        let key = derive_key(password, &salt, &params).unwrap();
        let ciphertext = encrypt(b"secret", &key, &nonce).unwrap();
        let file_bytes = format::write_file(CURRENT_VERSION, &params, &salt, &nonce, &ciphertext);

        let (header, ct) = format::read_file(&file_bytes).unwrap();
        let wrong_key = derive_key("wrong", &header.salt, &header.kdf_params).unwrap();
        assert!(decrypt(ct, &wrong_key, &header.nonce).is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════
// KDF bounds validation (T7) — crafted malicious headers
// ═══════════════════════════════════════════════════════════════════

mod kdf_bounds_validation {
    use savings_tracker_core::errors::CoreError;
    use savings_tracker_core::storage::format;

    /// Build a minimal valid .svtk byte array with the given KDF params.
    fn craft_bytes(memory_cost: u32, time_cost: u32, parallelism: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"SVTK");            // magic
        buf.extend_from_slice(&1u16.to_le_bytes()); // version 1
        buf.extend_from_slice(&memory_cost.to_le_bytes());
        buf.extend_from_slice(&time_cost.to_le_bytes());
        buf.extend_from_slice(&parallelism.to_le_bytes());
        buf.extend_from_slice(&[0u8; 16]);           // salt
        buf.extend_from_slice(&[0u8; 12]);           // nonce
        buf.extend_from_slice(&0u64.to_le_bytes());  // ciphertext_len = 0
        buf
    }

    #[test]
    fn reject_memory_cost_zero() {
        let bytes = craft_bytes(0, 3, 4);
        match format::read_file(&bytes) {
            Err(CoreError::InvalidFileFormat(msg)) => assert!(msg.contains("memory_cost"), "{msg}"),
            other => panic!("expected InvalidFileFormat for memory_cost 0, got: {other:?}"),
        }
    }

    #[test]
    fn reject_memory_cost_too_low() {
        let bytes = craft_bytes(7, 3, 4); // below minimum of 8
        assert!(format::read_file(&bytes).is_err());
    }

    #[test]
    fn reject_memory_cost_too_high() {
        let bytes = craft_bytes(2_000_000, 3, 4); // above 1_048_576
        match format::read_file(&bytes) {
            Err(CoreError::InvalidFileFormat(msg)) => assert!(msg.contains("memory_cost"), "{msg}"),
            other => panic!("expected InvalidFileFormat for huge memory_cost, got: {other:?}"),
        }
    }

    #[test]
    fn reject_memory_cost_u32_max() {
        let bytes = craft_bytes(u32::MAX, 3, 4);
        assert!(format::read_file(&bytes).is_err());
    }

    #[test]
    fn reject_time_cost_zero() {
        let bytes = craft_bytes(65_536, 0, 4);
        match format::read_file(&bytes) {
            Err(CoreError::InvalidFileFormat(msg)) => assert!(msg.contains("time_cost"), "{msg}"),
            other => panic!("expected InvalidFileFormat for time_cost 0, got: {other:?}"),
        }
    }

    #[test]
    fn reject_time_cost_too_high() {
        let bytes = craft_bytes(65_536, 21, 4); // above max 20
        match format::read_file(&bytes) {
            Err(CoreError::InvalidFileFormat(msg)) => assert!(msg.contains("time_cost"), "{msg}"),
            other => panic!("expected InvalidFileFormat for time_cost 21, got: {other:?}"),
        }
    }

    #[test]
    fn reject_time_cost_u32_max() {
        let bytes = craft_bytes(65_536, u32::MAX, 4);
        assert!(format::read_file(&bytes).is_err());
    }

    #[test]
    fn reject_parallelism_zero() {
        let bytes = craft_bytes(65_536, 3, 0);
        match format::read_file(&bytes) {
            Err(CoreError::InvalidFileFormat(msg)) => assert!(msg.contains("parallelism"), "{msg}"),
            other => panic!("expected InvalidFileFormat for parallelism 0, got: {other:?}"),
        }
    }

    #[test]
    fn reject_parallelism_too_high() {
        let bytes = craft_bytes(65_536, 3, 17); // above max 16
        match format::read_file(&bytes) {
            Err(CoreError::InvalidFileFormat(msg)) => assert!(msg.contains("parallelism"), "{msg}"),
            other => panic!("expected InvalidFileFormat for parallelism 17, got: {other:?}"),
        }
    }

    #[test]
    fn reject_parallelism_u32_max() {
        let bytes = craft_bytes(65_536, 3, u32::MAX);
        assert!(format::read_file(&bytes).is_err());
    }

    #[test]
    fn accept_valid_boundary_low() {
        // All at minimum valid values: memory=8, time=1, parallelism=1
        let bytes = craft_bytes(8, 1, 1);
        assert!(format::read_file(&bytes).is_ok());
    }

    #[test]
    fn accept_valid_boundary_high() {
        // All at maximum valid values: memory=1_048_576, time=20, parallelism=16
        let bytes = craft_bytes(1_048_576, 20, 16);
        assert!(format::read_file(&bytes).is_ok());
    }

    #[test]
    fn accept_default_values() {
        // Default: memory=65_536, time=3, parallelism=4
        let bytes = craft_bytes(65_536, 3, 4);
        assert!(format::read_file(&bytes).is_ok());
    }
}

