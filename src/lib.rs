//! Nano ID generation with caller-provided cryptographically secure randomness.
//!
//! The raw WebAssembly ABI imports `env.random_fill(pointer, length) -> i32`.
//! The host must fill the provided range with CSPRNG bytes and return zero.

use std::string::String;

/// Nano ID's standard URL-safe alphabet.
pub const DEFAULT_ALPHABET: &[u8; 64] =
    b"_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Maximum output length accepted by the raw WebAssembly ABI.
pub const MAX_LENGTH: usize = 1_048_576;
/// Maximum UTF-8 exclusion input accepted by the raw WebAssembly ABI.
pub const MAX_EXCLUDED_BYTES: usize = 1_048_576;
const RANDOM_BATCH: usize = 64;

/// Errors returned by [`generate_with_rng`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidUtf8,
    LengthTooLarge,
    EmptyAlphabet,
    RandomSourceFailed,
}

fn validate_excluded(bytes: &[u8]) -> Result<&str, Error> {
    std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
}

/// Generate an ID using a caller-supplied source of uniformly random bytes.
///
/// `excluded` is UTF-8 text. Each Unicode scalar value in it is excluded from
/// the alphabet. Since Nano ID's default alphabet is ASCII, non-ASCII
/// exclusions are valid but do not remove any alphabet symbols.
///
/// The random callback must fill the provided slice with independent,
/// uniformly random bytes. Use a cryptographically secure source in production.
pub fn generate_with_rng<F>(
    length: usize,
    excluded: &str,
    mut fill_random: F,
) -> Result<String, Error>
where
    F: FnMut(&mut [u8]) -> Result<(), ()>,
{
    if length > MAX_LENGTH {
        return Err(Error::LengthTooLarge);
    }

    // Use scalar values rather than bytes so multibyte exclusions are parsed
    // correctly. The alphabet itself contains only ASCII bytes.
    let excluded: Vec<char> = excluded.chars().collect();
    let alphabet: Vec<u8> = DEFAULT_ALPHABET
        .iter()
        .copied()
        .filter(|byte| !excluded.contains(&char::from(*byte)))
        .collect();
    if alphabet.is_empty() {
        return Err(Error::EmptyAlphabet);
    }

    let n = alphabet.len() as u16;
    let limit = (256 / n) * n;
    let mut output = String::with_capacity(length);
    let mut random = [0u8; RANDOM_BATCH];
    let mut cursor = RANDOM_BATCH;
    while output.len() < length {
        if cursor == random.len() {
            fill_random(&mut random).map_err(|_| Error::RandomSourceFailed)?;
            cursor = 0;
        }
        let value = random[cursor] as u16;
        cursor += 1;
        if value < limit {
            output.push(char::from(alphabet[(value % n) as usize]));
        }
    }
    Ok(output)
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::{generate_with_rng, DEFAULT_ALPHABET, MAX_EXCLUDED_BYTES, MAX_LENGTH};
    use std::alloc::{alloc, dealloc, Layout};
    use std::ptr;
    use std::slice;

    #[link(wasm_import_module = "env")]
    extern "C" {
        fn random_fill(output_ptr: *mut u8, length: u32) -> i32;
    }

    /// Allocate a host-owned input or output buffer in the Rust allocator.
    /// Returns null if allocation fails. Pass the same length to `free_buffer`.
    #[no_mangle]
    pub extern "C" fn alloc_buffer(length: u32) -> *mut u8 {
        let layout = match Layout::array::<u8>((length as usize).max(1)) {
            Ok(layout) => layout,
            Err(_) => return ptr::null_mut(),
        };
        // SAFETY: Layout is valid and the returned block is owned by the caller
        // until it is passed back to `free_buffer` with the same length.
        unsafe { alloc(layout) }
    }

    /// Release a buffer returned by `alloc_buffer`.
    #[no_mangle]
    pub unsafe extern "C" fn free_buffer(pointer: *mut u8, length: u32) {
        if pointer.is_null() {
            return;
        }
        let layout = Layout::array::<u8>((length as usize).max(1))
            .expect("the layout was validated by alloc_buffer");
        // SAFETY: The caller must pass the original pointer and size from
        // `alloc_buffer` exactly once.
        dealloc(pointer, layout);
    }

    /// Generate a Nano ID into caller-owned linear memory.
    ///
    /// Returns 0 on success, or a negative error code. The caller must ensure
    /// the ranges are allocator-owned, inside exported memory, and do not overlap.
    ///
    /// # Safety
    /// Pointers must refer to valid ranges in WebAssembly linear memory.
    #[no_mangle]
    pub unsafe extern "C" fn generate(
        output_ptr: *mut u8,
        length: u32,
        excluded_ptr: *const u8,
        excluded_len: u32,
    ) -> i32 {
        let length = length as usize;
        let excluded_len = excluded_len as usize;
        if length > MAX_LENGTH {
            return -2;
        }
        if excluded_len > MAX_EXCLUDED_BYTES {
            return -6;
        }
        if (length > 0 && output_ptr.is_null()) || (excluded_len > 0 && excluded_ptr.is_null()) {
            return -5;
        }
        if length > 0 && excluded_len > 0 {
            let out_start = output_ptr as usize;
            let excluded_start = excluded_ptr as usize;
            let out_end = match out_start.checked_add(length) {
                Some(end) => end,
                None => return -5,
            };
            let excluded_end = match excluded_start.checked_add(excluded_len) {
                Some(end) => end,
                None => return -5,
            };
            if out_start < excluded_end && excluded_start < out_end {
                return -5;
            }
        }

        let excluded_bytes = if excluded_len == 0 {
            &[][..]
        } else {
            slice::from_raw_parts(excluded_ptr, excluded_len)
        };
        let excluded = match super::validate_excluded(excluded_bytes) {
            Ok(value) => value,
            Err(_) => return -1,
        };

        // Fail before writing output if the excluded set removes the entire
        // alphabet. An invalid byte from the host RNG is also rejected.
        if DEFAULT_ALPHABET
            .iter()
            .all(|b| excluded.contains(char::from(*b)))
        {
            return -3;
        }

        let generated = match generate_with_rng(length, excluded, |buffer| {
            // SAFETY: `buffer` is a live stack range in linear memory. The
            // host fills it with cryptographically secure random bytes.
            let status = random_fill(buffer.as_mut_ptr(), buffer.len() as u32);
            if status != 0 {
                return Err(());
            }
            Ok(())
        }) {
            Ok(value) => value,
            Err(super::Error::RandomSourceFailed) => return -4,
            Err(super::Error::EmptyAlphabet) => return -3,
            Err(super::Error::LengthTooLarge) => return -2,
            Err(super::Error::InvalidUtf8) => return -1,
        };
        if length > 0 {
            ptr::copy_nonoverlapping(generated.as_ptr(), output_ptr, length);
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{generate_with_rng, validate_excluded, Error, DEFAULT_ALPHABET, MAX_LENGTH};

    #[test]
    fn produces_requested_length_and_uses_available_symbols() {
        let mut next = 0u8;
        let id = generate_with_rng(64, "_", |out| {
            for byte in out {
                *byte = next;
                next = next.wrapping_add(1);
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(id.chars().count(), 64);
        assert!(!id.contains('_'));
        assert!(id.chars().all(|c| DEFAULT_ALPHABET.contains(&(c as u8))));
    }

    #[test]
    fn non_ascii_exclusion_is_a_valid_scalar_and_does_not_corrupt_alphabet() {
        let id = generate_with_rng(32, "🦀é", |bytes| {
            bytes.fill(0);
            Ok(())
        })
        .unwrap();
        assert_eq!(id.len(), 32);
        assert!(id.is_ascii());
    }

    #[test]
    fn rejects_excluding_the_entire_alphabet() {
        let excluded: String = DEFAULT_ALPHABET.iter().map(|b| *b as char).collect();
        assert_eq!(
            generate_with_rng(1, &excluded, |_| Ok(())),
            Err(Error::EmptyAlphabet)
        );
    }

    #[test]
    fn checks_length_bound_and_accepts_zero_length() {
        assert_eq!(
            generate_with_rng(MAX_LENGTH + 1, "", |_| Ok(())),
            Err(Error::LengthTooLarge)
        );
        assert_eq!(generate_with_rng(0, "", |_| Ok(())).unwrap(), "");
    }

    #[test]
    fn rejects_rng_failure() {
        assert_eq!(
            generate_with_rng(1, "", |_| Err(())),
            Err(Error::RandomSourceFailed)
        );
    }

    #[test]
    fn rejection_sampling_skips_the_incomplete_random_byte_bucket() {
        let mut batches = 0;
        let id = generate_with_rng(1, "A", |bytes| {
            bytes.fill(if batches == 0 { 255 } else { 0 });
            batches += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(batches, 2);
        assert_eq!(id, "_");
    }

    #[test]
    fn rejects_invalid_utf8_exclusion_bytes() {
        assert_eq!(
            validate_excluded(&[0xf0, 0x28, 0x8c, 0x28]),
            Err(Error::InvalidUtf8)
        );
    }
}
