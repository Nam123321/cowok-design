// @story 06.1
// @implements AC-02, AC-05
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
static SEQ: AtomicU16 = AtomicU16::new(0);

/// Generates a 30-character Crockford Base32 ULID matching `^SLD-[0-9A-HJKMNP-TV-Z]{26}$`.
pub fn generate_slide_uid() -> String {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let rand_bytes = *Uuid::new_v4().as_bytes();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);

    let mut raw = [0u8; 16];
    let ts_be = ts_ms.to_be_bytes();
    raw[0..6].copy_from_slice(&ts_be[2..8]);
    raw[6..16].copy_from_slice(&rand_bytes[6..16]);
    raw[14] ^= (seq >> 8) as u8;
    raw[15] ^= (seq & 0xFF) as u8;

    let val = u128::from_be_bytes(raw);
    let mut chars = [0u8; 26];
    for (idx, slot) in chars.iter_mut().enumerate() {
        let shift = (25 - idx) * 5;
        let digit = ((val >> shift) & 0x1F) as usize;
        *slot = CROCKFORD[digit];
    }
    let suffix = std::str::from_utf8(&chars).unwrap_or("00000000000000000000000000");
    format!("SLD-{suffix}")
}

pub fn is_valid_slide_uid(uid: &str) -> bool {
    uid.len() == 30
        && uid
            .strip_prefix("SLD-")
            .is_some_and(|rest| rest.bytes().all(|b| CROCKFORD.contains(&b)))
}
