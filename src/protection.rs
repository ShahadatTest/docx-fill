//! Password edit protection: `<w:documentProtection>` with the password hash Word checks. Built
//! from ECMA-376 Part 4 §2.15.1.28 and FIPS 180-4.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::FillError;

/// What a protected document lets a reader change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionKind {
    ReadOnly,
    TrackedChanges,
    Forms,
    Comments,
}

impl ProtectionKind {
    fn value(self) -> &'static str {
        match self {
            ProtectionKind::ReadOnly => "readOnly",
            ProtectionKind::TrackedChanges => "trackedChanges",
            ProtectionKind::Forms => "forms",
            ProtectionKind::Comments => "comments",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlgorithm {
    /// The provider type and algorithm id Word writes for the algorithm.
    fn provider(self) -> (&'static str, u32) {
        match self {
            HashAlgorithm::Sha1 => ("rsaFull", 4),
            HashAlgorithm::Sha256 => ("rsaAES", 12),
            HashAlgorithm::Sha384 => ("rsaAES", 13),
            HashAlgorithm::Sha512 => ("rsaAES", 14),
        }
    }

    fn hash(self, data: &[u8]) -> Vec<u8> {
        match self {
            HashAlgorithm::Sha1 => sha1(data).to_vec(),
            HashAlgorithm::Sha256 => sha256(data).to_vec(),
            HashAlgorithm::Sha384 => sha512_with(data, &SHA384_START)[..48].to_vec(),
            HashAlgorithm::Sha512 => sha512_with(data, &SHA512_START).to_vec(),
        }
    }
}

/// Restricts editing behind a password Word asks for to lift it. Word respects the restriction;
/// the content is not encrypted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protection {
    /// At least 6 characters.
    pub password: String,
    pub kind: ProtectionKind,
    pub algorithm: HashAlgorithm,
    pub spin_count: u32,
    /// Base64. A new one for each fill when `None`.
    pub salt: Option<String>,
}

impl Protection {
    /// Read-only protection with SHA-512 and 100000 spins, as Word writes it.
    pub fn new(password: impl Into<String>) -> Protection {
        Protection { password: password.into(), kind: ProtectionKind::ReadOnly, algorithm: HashAlgorithm::Sha512, spin_count: 100_000, salt: None }
    }

    pub(crate) fn check(&self) -> Result<(), FillError> {
        if self.password.chars().count() < MIN_PASSWORD_LENGTH {
            return Err(FillError::new("The password length must be of at least 6 characters"));
        }
        if self.salt.as_deref().is_some_and(|salt| decode_base64(salt).is_none()) {
            return Err(FillError::new("The salt is not base64"));
        }
        Ok(())
    }

    /// The `<w:documentProtection>` element.
    pub(crate) fn element(&self) -> String {
        let salt = self.salt.as_deref().and_then(decode_base64).unwrap_or_else(|| new_salt(&self.password));
        let hash = hash_password(&self.password, self.algorithm, &salt, self.spin_count);
        let (provider, id) = self.algorithm.provider();
        format!(
            "<w:documentProtection w:edit=\"{}\" w:enforcement=\"1\" w:cryptProviderType=\"{provider}\" w:cryptAlgorithmClass=\"hash\" w:cryptAlgorithmType=\"typeAny\" w:cryptAlgorithmSid=\"{id}\" w:cryptSpinCount=\"{}\" w:hash=\"{}\" w:salt=\"{}\"/>",
            self.kind.value(),
            self.spin_count,
            encode_base64(&hash),
            encode_base64(&salt)
        )
    }
}

const MIN_PASSWORD_LENGTH: usize = 6;
// Word's legacy key reads at most this many characters.
const MAX_LEGACY_LENGTH: usize = 15;
const SALT_LENGTH: usize = 16;

const INITIAL_CODES: [u16; 15] = [0xE1F0, 0x1D0F, 0xCC9C, 0x84C0, 0x110C, 0x0E10, 0xF1CE, 0x313E, 0x1872, 0xE139, 0xD40F, 0x84F9, 0x280C, 0xA96A, 0x4EC3];
// The first column of the spec's encryption matrix; each row's next entry is the CRC-16 step of
// the one before.
const MATRIX_STARTS: [u16; 15] = [0xAEFC, 0x7B61, 0x4563, 0x0375, 0xD849, 0x6F45, 0xEB23, 0x47D3, 0xB861, 0x45A0, 0xAA51, 0x76B4, 0x3730, 0x3331, 0x1021];

/// Word's hash of `password`: the legacy 32-bit key as reversed hex, hashed in UTF-16LE after the
/// salt, then hashed `spin_count` times more with each iteration number appended.
fn hash_password(password: &str, algorithm: HashAlgorithm, salt: &[u8], spin_count: u32) -> Vec<u8> {
    let key = legacy_key(password).to_le_bytes();
    let hex: String = key.iter().map(|byte| format!("{byte:02X}")).collect();
    let mut input = salt.to_vec();
    input.extend(hex.encode_utf16().flat_map(u16::to_le_bytes));
    let mut hash = algorithm.hash(&input);
    for iteration in 0..spin_count {
        hash.extend(iteration.to_le_bytes());
        hash = algorithm.hash(&hash);
    }
    hash
}

/// The 32-bit key Word derives from up to 15 of the password's characters.
fn legacy_key(password: &str) -> u32 {
    let bytes: Vec<u8> = password
        .encode_utf16()
        .take(MAX_LEGACY_LENGTH)
        .map(|unit| match (unit & 0xFF) as u8 {
            0 => (unit >> 8) as u8,
            low => low,
        })
        .collect();
    if bytes.is_empty() {
        return 0;
    }

    let mut high = INITIAL_CODES[bytes.len() - 1];
    for (index, byte) in bytes.iter().enumerate() {
        let mut entry = MATRIX_STARTS[MAX_LEGACY_LENGTH - bytes.len() + index];
        for bit in 0..7 {
            if byte & (1 << bit) != 0 {
                high ^= entry;
            }
            entry = (entry << 1) ^ if entry & 0x8000 != 0 { 0x1021 } else { 0 };
        }
    }

    let rotate = |value: u16| ((value >> 14) & 1) | ((value << 1) & 0x7FFF);
    let mut low = 0u16;
    for byte in bytes.iter().rev() {
        low = rotate(low) ^ u16::from(*byte);
    }
    low = rotate(low) ^ bytes.len() as u16 ^ 0xCE4B;
    (u32::from(high) << 16) | u32::from(low)
}

/// A salt unlikely to repeat. Not secret, so a seeded hash is enough.
fn new_salt(password: &str) -> Vec<u8> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write(password.as_bytes());
    let mut seed = hasher.finish().to_le_bytes().to_vec();
    seed.extend((&COUNTER as *const AtomicU64 as usize).to_le_bytes());
    HashAlgorithm::Sha512.hash(&seed)[..SALT_LENGTH].to_vec()
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut state: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    for block in pad(data, 64, 8).chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes(word.try_into().expect("4 bytes"));
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let next = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*word);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, next);
        }
        for (value, added) in state.iter_mut().zip([a, b, c, d, e]) {
            *value = value.wrapping_add(added);
        }
    }
    let mut out = [0u8; 20];
    for (chunk, value) in out.chunks_mut(4).zip(state) {
        chunk.copy_from_slice(&value.to_be_bytes());
    }
    out
}

const SHA256_ROUNDS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    for block in pad(data, 64, 8).chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes(word.try_into().expect("4 bytes"));
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = state;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let choice = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let first = v[7].wrapping_add(s1).wrapping_add(choice).wrapping_add(SHA256_ROUNDS[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let majority = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let second = s0.wrapping_add(majority);
            v = [first.wrapping_add(second), v[0], v[1], v[2], v[3].wrapping_add(first), v[4], v[5], v[6]];
        }
        for (value, added) in state.iter_mut().zip(v) {
            *value = value.wrapping_add(added);
        }
    }
    let mut out = [0u8; 32];
    for (chunk, value) in out.chunks_mut(4).zip(state) {
        chunk.copy_from_slice(&value.to_be_bytes());
    }
    out
}

const SHA512_START: [u64; 8] = [
    0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1, 0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
];
const SHA384_START: [u64; 8] = [
    0xcbbb9d5dc1059ed8, 0x629a292a367cd507, 0x9159015a3070dd17, 0x152fecd8f70e5939, 0x67332667ffc00b31, 0x8eb44a8768581511, 0xdb0c2e0d64f98fa7, 0x47b5481dbefa4fa4,
];
const SHA512_ROUNDS: [u64; 80] = [
    0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc, 0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b,
    0xab1c5ed5da6d8118, 0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2, 0x72be5d74f27b896f, 0x80deb1fe3b1696b1,
    0x9bdc06a725c71235, 0xc19bf174cf692694, 0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65, 0x2de92c6f592b0275,
    0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5, 0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4,
    0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70, 0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed,
    0x53380d139d95b3df, 0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b, 0xa2bfe8a14cf10364, 0xa81a664bbc423001,
    0xc24b8b70d0f89791, 0xc76c51a30654be30, 0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8, 0x19a4c116b8d2d0c8,
    0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8, 0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec, 0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915,
    0xc67178f2e372532b, 0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178, 0x06f067aa72176fba, 0x0a637dc5a2c898a6,
    0x113f9804bef90dae, 0x1b710b35131c471b, 0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c, 0x4cc5d4becb3e42b6,
    0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
];

fn sha512_with(data: &[u8], start: &[u64; 8]) -> [u8; 64] {
    let mut state = *start;
    for block in pad(data, 128, 16).chunks(128) {
        let mut w = [0u64; 80];
        for (i, word) in block.chunks(8).enumerate() {
            w[i] = u64::from_be_bytes(word.try_into().expect("8 bytes"));
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = state;
        for i in 0..80 {
            let s1 = v[4].rotate_right(14) ^ v[4].rotate_right(18) ^ v[4].rotate_right(41);
            let choice = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let first = v[7].wrapping_add(s1).wrapping_add(choice).wrapping_add(SHA512_ROUNDS[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(28) ^ v[0].rotate_right(34) ^ v[0].rotate_right(39);
            let majority = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let second = s0.wrapping_add(majority);
            v = [first.wrapping_add(second), v[0], v[1], v[2], v[3].wrapping_add(first), v[4], v[5], v[6]];
        }
        for (value, added) in state.iter_mut().zip(v) {
            *value = value.wrapping_add(added);
        }
    }
    let mut out = [0u8; 64];
    for (chunk, value) in out.chunks_mut(8).zip(state) {
        chunk.copy_from_slice(&value.to_be_bytes());
    }
    out
}

/// `data` padded to whole `block`-byte blocks: a 1 bit, zeros, then its bit length in
/// `length_bytes` big-endian bytes.
fn pad(data: &[u8], block: usize, length_bytes: usize) -> Vec<u8> {
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % block != block - length_bytes {
        padded.push(0);
    }
    let bits = (data.len() as u128) * 8;
    padded.extend(&bits.to_be_bytes()[16 - length_bytes..]);
    padded
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(BASE64[((value >> (18 - 6 * index)) & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let text = text.trim().trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut value, mut bits) = (0u32, 0);
    for c in text.bytes() {
        value = (value << 6) | BASE64.iter().position(|each| *each == c)? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((value >> bits) as u8);
            value &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn hashes_match_fips_examples() {
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hex(&HashAlgorithm::Sha384.hash(b"abc")), "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7");
        assert_eq!(
            hex(&HashAlgorithm::Sha512.hash(b"abc")),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        let long = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(hex(&sha256(long)), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    }

    // From black-box runs of Apache POI's CryptoFunctions, salt bytes 0 to 15, 100000 spins.
    // ASCII only: POI sign-extends bytes over 0x7F, where the spec's key reads them unsigned.
    #[test]
    fn hashes_passwords_as_word_checks_them() {
        let salt: Vec<u8> = (0..16).collect();
        let cases = [
            ("password", "AF837A14", HashAlgorithm::Sha512, "4rOTG/MwYlSJ4565z4hrF8V/7lr1APYteXrflsxTHLojytNOf6N6lINftoVjzehQpL6IZBhd3SdxNAqIXd/mAQ=="),
            ("password", "AF837A14", HashAlgorithm::Sha1, "QzuOhrRyol7WPaeLV9/sWyuJCw0="),
            ("Scope2026!", "328928FB", HashAlgorithm::Sha256, "R5gYuCZD03npMzJp4UurhpH9Lkr8oXc+8kqwu1GGNEI="),
            ("abcdefghijklmnopqrstu", "BCC6055A", HashAlgorithm::Sha384, "KbaJK5ah+hxe7uSGxA3JdWeX8/AixB4MQZ2aBGS0TXOWZdOyuvhZBJnPBi5uvinU"),
        ];
        for (password, key, algorithm, hash) in cases {
            let reversed: String = legacy_key(password).to_le_bytes().iter().map(|byte| format!("{byte:02X}")).collect();
            assert_eq!(reversed, key, "{password}");
            assert_eq!(encode_base64(&hash_password(password, algorithm, &salt, 100_000)), hash, "{password}");
        }
    }

    #[test]
    fn round_trips_base64() {
        for bytes in [&b""[..], b"f", b"fo", b"foo", b"foob"] {
            assert_eq!(decode_base64(&encode_base64(bytes)).unwrap(), bytes);
        }
        assert_eq!(encode_base64(b"foob"), "Zm9vYg==");
    }
}
