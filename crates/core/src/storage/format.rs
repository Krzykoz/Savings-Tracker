use crate::errors::CoreError;
use super::encryption::KdfParams;

/// Magic bytes identifying an SVTK (Savings Tracker) file.
pub const MAGIC: &[u8; 4] = b"SVTK";

/// Current file format version.
pub const CURRENT_VERSION: u16 = 1;

/// Minimum header size in bytes:
/// magic(4) + version(2) + kdf_params(12) + salt(16) + nonce(12) + ciphertext_len(8) = 54
pub const MIN_HEADER_SIZE: usize = 54;

/// File header read from an encrypted .svtk file.
#[derive(Debug)]
pub struct FileHeader {
    pub version: u16,
    pub kdf_params: KdfParams,
    pub salt: [u8; 16],
    pub nonce: [u8; 12],
    pub ciphertext_len: u64,
}

/// Write a complete encrypted file to bytes.
///
/// Layout:
/// ```text
/// [SVTK: 4B] [version: 2B LE] [memory_cost: 4B LE] [time_cost: 4B LE]
/// [parallelism: 4B LE] [salt: 16B] [nonce: 12B] [ciphertext_len: 8B LE]
/// [ciphertext: variable]
/// ```
pub fn write_file(
    version: u16,
    kdf_params: &KdfParams,
    salt: &[u8; 16],
    nonce: &[u8; 12],
    ciphertext: &[u8],
) -> Vec<u8> {
    let ciphertext_len = ciphertext.len() as u64;
    let total_size = MIN_HEADER_SIZE + ciphertext.len();
    let mut buf = Vec::with_capacity(total_size);

    // Magic
    buf.extend_from_slice(MAGIC);
    // Version
    buf.extend_from_slice(&version.to_le_bytes());
    // KDF params
    buf.extend_from_slice(&kdf_params.memory_cost.to_le_bytes());
    buf.extend_from_slice(&kdf_params.time_cost.to_le_bytes());
    buf.extend_from_slice(&kdf_params.parallelism.to_le_bytes());
    // Salt
    buf.extend_from_slice(salt);
    // Nonce
    buf.extend_from_slice(nonce);
    // Ciphertext length
    buf.extend_from_slice(&ciphertext_len.to_le_bytes());
    // Ciphertext (includes AES-GCM auth tag)
    buf.extend_from_slice(ciphertext);

    buf
}

/// Parse the header from raw file bytes.
/// Returns the header and the ciphertext slice.
pub fn read_file(data: &[u8]) -> Result<(FileHeader, &[u8]), CoreError> {
    if data.len() < MIN_HEADER_SIZE {
        return Err(CoreError::InvalidFileFormat(
            "File too small to be a valid SVTK file".into(),
        ));
    }

    // Validate magic bytes
    if &data[0..4] != MAGIC {
        return Err(CoreError::InvalidFileFormat(
            "Invalid magic bytes — not an SVTK file".into(),
        ));
    }

    let mut offset = 4;

    // Version
    let version = u16::from_le_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    if version == 0 || version > CURRENT_VERSION {
        return Err(CoreError::UnsupportedVersion(version));
    }

    // KDF params
    let memory_cost = u32::from_le_bytes(
        data[offset..offset + 4].try_into().map_err(|_| {
            CoreError::InvalidFileFormat("Failed to read KDF memory_cost".into())
        })?,
    );
    offset += 4;
    let time_cost = u32::from_le_bytes(
        data[offset..offset + 4].try_into().map_err(|_| {
            CoreError::InvalidFileFormat("Failed to read KDF time_cost".into())
        })?,
    );
    offset += 4;
    let parallelism = u32::from_le_bytes(
        data[offset..offset + 4].try_into().map_err(|_| {
            CoreError::InvalidFileFormat("Failed to read KDF parallelism".into())
        })?,
    );
    offset += 4;

    // Validate KDF params to prevent resource-exhaustion attacks from crafted files.
    // memory_cost: max 1 GiB (1_048_576 KiB), min 8 KiB (Argon2 minimum)
    // time_cost: max 20 iterations
    // parallelism: max 16 threads, min 1
    if !(8..=1_048_576).contains(&memory_cost) {
        return Err(CoreError::InvalidFileFormat(format!(
            "KDF memory_cost out of safe range: {memory_cost} KiB (expected 8..1048576)"
        )));
    }
    if !(1..=20).contains(&time_cost) {
        return Err(CoreError::InvalidFileFormat(format!(
            "KDF time_cost out of safe range: {time_cost} (expected 1..20)"
        )));
    }
    if !(1..=16).contains(&parallelism) {
        return Err(CoreError::InvalidFileFormat(format!(
            "KDF parallelism out of safe range: {parallelism} (expected 1..16)"
        )));
    }

    // Salt
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&data[offset..offset + 16]);
    offset += 16;

    // Nonce
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&data[offset..offset + 12]);
    offset += 12;

    // Ciphertext length
    let ciphertext_len = u64::from_le_bytes(
        data[offset..offset + 8].try_into().map_err(|_| {
            CoreError::InvalidFileFormat("Failed to read ciphertext length".into())
        })?,
    );
    offset += 8;

    let expected_end = offset + ciphertext_len as usize;
    if data.len() < expected_end {
        return Err(CoreError::InvalidFileFormat(format!(
            "File truncated: expected {} bytes of ciphertext, got {}",
            ciphertext_len,
            data.len() - offset
        )));
    }

    let ciphertext = &data[offset..expected_end];

    let header = FileHeader {
        version,
        kdf_params: KdfParams {
            memory_cost,
            time_cost,
            parallelism,
        },
        salt,
        nonce,
        ciphertext_len,
    };

    Ok((header, ciphertext))
}
