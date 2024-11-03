use std::fmt::Write;
use std::num::NonZeroU32;

use ring::{
    pbkdf2,
    rand::{SecureRandom, SystemRandom},
};

use crate::utils::{err, Res};

const KEY_LENGTH: usize = ring::digest::SHA256_OUTPUT_LEN;
pub type Key = [u8; KEY_LENGTH];

pub fn generate_salt() -> Res<Key> {
    let mut bytes = [0u8; KEY_LENGTH];
    let rng = SystemRandom::new();
    match rng.fill(&mut bytes) {
        Ok(()) => Ok(bytes),
        Err(_) => err("Random byte generation failed."),
    }
}

pub fn to_hex_string(key: &Key) -> Res<String> {
    let mut s = String::with_capacity(KEY_LENGTH * 2);
    for byte in *key {
        write!(s, "{:02X}", byte).map_err(|e| e.to_string())?;
    }

    Ok(s)
}

pub fn format_hex(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for &byte in data {
        // Can't fail as s has sufficient capacity.
        write!(s, "{:02x}", byte).unwrap();
    }
    s
}
pub fn to_hex_string_unsized(data: &[u8]) -> Res<String> {
    let key = &<Key>::try_from(data).map_err(|e| format!("Failed to convert Vec to Key: {e}"))?;
    to_hex_string(key)
}

pub fn from_hex_string(string: &str) -> Res<Key> {
    let err = err("Failed to decode hex.");
    match ring::test::from_hex(string) {
        Ok(v) => Ok(v.try_into().or(err)?),
        Err(_) => err,
    }
}

pub fn random_hex_string(length: usize) -> Res<String> {
    Ok(to_hex_string(&generate_salt()?)?[..length].to_string())
}

const ITERATIONS: u32 = 10_000;
pub fn hash_password(salt: &Key, password: &str) -> Key {
    let mut hashed = [0u8; KEY_LENGTH];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(ITERATIONS).unwrap(),
        salt,
        password.as_bytes(),
        &mut hashed,
    );

    hashed
}

pub fn check_password(provided: &str, salt: &Key, hashed_password: &Key) -> bool {
    pbkdf2::verify(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(ITERATIONS).unwrap(),
        salt,
        provided.as_bytes(),
        hashed_password,
    )
    .is_ok()
}
