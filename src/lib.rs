//! Nano ID generation with caller-provided cryptographically secure randomness.
//!
//! The raw WebAssembly ABI imports `env.random_fill(pointer, length) -> i32`.
//! The host must fill the provided range with CSPRNG bytes and return zero.

use std::collections::HashSet;
use std::string::String;

/// Nano ID's standard URL-safe alphabet.
pub const DEFAULT_ALPHABET: &[u8; 64] =
    b"_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Maximum output length accepted by the raw WebAssembly ABI.
pub const MAX_LENGTH: usize = 1_048_576;
/// Maximum UTF-8 exclusion input accepted by the raw WebAssembly ABI.
pub const MAX_EXCLUDED_BYTES: usize = 1_048_576;
/// Maximum UTF-8 custom alphabet size accepted by the raw WebAssembly ABI.
pub const MAX_ALPHABET_BYTES: usize = 1_048_576;
const RANDOM_BATCH: usize = 256;

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

/// Generate an ID from a custom UTF-8 alphabet using caller-provided randomness.
/// `length` counts Unicode scalar values, not UTF-8 bytes or grapheme clusters.
/// Repeated alphabet symbols are deduplicated; every remaining symbol has equal
/// probability. The random callback must fill its buffer with independent,
/// uniformly random bytes. Use a cryptographically secure source in production.
pub fn generate_with_alphabet_and_rng<F>(
    length: usize,
    excluded: &str,
    alphabet: &str,
    mut fill_random: F,
) -> Result<String, Error>
where
    F: FnMut(&mut [u8]) -> Result<(), ()>,
{
    if length > MAX_LENGTH {
        return Err(Error::LengthTooLarge);
    }

    if alphabet.len() > MAX_ALPHABET_BYTES {
        return Err(Error::LengthTooLarge);
    }

    let excluded: HashSet<char> = excluded.chars().collect();
    let mut seen = HashSet::new();
    let symbols: Vec<char> = alphabet
        .chars()
        .filter(|symbol| seen.insert(*symbol) && !excluded.contains(symbol))
        .collect();
    if symbols.is_empty() {
        return Err(Error::EmptyAlphabet);
    }

    let n = symbols.len() as u64;
    let range = 1u64 << 32;
    let limit = range - range % n;
    let mut output = String::with_capacity(length);
    let mut generated_symbols = 0;
    let mut random = [0u8; RANDOM_BATCH];
    let mut cursor = RANDOM_BATCH;
    while generated_symbols < length {
        if cursor == random.len() {
            fill_random(&mut random).map_err(|_| Error::RandomSourceFailed)?;
            cursor = 0;
        }
        let value = u32::from_le_bytes([
            random[cursor],
            random[cursor + 1],
            random[cursor + 2],
            random[cursor + 3],
        ]) as u64;
        cursor += 4;
        if value < limit {
            output.push(symbols[(value % n) as usize]);
            generated_symbols += 1;
        }
    }
    Ok(output)
}

/// Generate from Nano ID's default URL-safe alphabet.
pub fn generate_with_rng<F>(length: usize, excluded: &str, fill_random: F) -> Result<String, Error>
where
    F: FnMut(&mut [u8]) -> Result<(), ()>,
{
    let alphabet = std::str::from_utf8(DEFAULT_ALPHABET).expect("default alphabet is ASCII");
    generate_with_alphabet_and_rng(length, excluded, alphabet, fill_random)
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::{
        generate_with_alphabet_and_rng, DEFAULT_ALPHABET, MAX_ALPHABET_BYTES, MAX_EXCLUDED_BYTES,
        MAX_LENGTH,
    };
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
        let status = generate_with_alphabet(
            output_ptr,
            length,
            length,
            excluded_ptr,
            excluded_len,
            DEFAULT_ALPHABET.as_ptr(),
            DEFAULT_ALPHABET.len() as u32,
        );
        if status >= 0 {
            0
        } else {
            status
        }
    }

    /// Generate UTF-8 output from a caller-provided UTF-8 alphabet.
    /// `length` counts Unicode scalar values; allocate up to `length * 4` output
    /// bytes and pass that byte capacity. Returns 0 on success, or a negative
    /// error code. Input/output ranges must be valid and must not overlap.
    ///
    /// # Safety
    /// Each pointer must refer to a valid range in WebAssembly linear memory.
    #[no_mangle]
    pub unsafe extern "C" fn generate_with_alphabet(
        output_ptr: *mut u8,
        output_capacity: u32,
        length: u32,
        excluded_ptr: *const u8,
        excluded_len: u32,
        alphabet_ptr: *const u8,
        alphabet_len: u32,
    ) -> i32 {
        let capacity = output_capacity as usize;
        let length = length as usize;
        let excluded_len = excluded_len as usize;
        let alphabet_len = alphabet_len as usize;
        if length > MAX_LENGTH {
            return -2;
        }
        if excluded_len > MAX_EXCLUDED_BYTES {
            return -6;
        }
        if alphabet_len > MAX_ALPHABET_BYTES {
            return -8;
        }
        if (capacity > 0 && output_ptr.is_null())
            || (excluded_len > 0 && excluded_ptr.is_null())
            || (alphabet_len > 0 && alphabet_ptr.is_null())
        {
            return -5;
        }

        let out_start = output_ptr as usize;
        let out_end = match out_start.checked_add(capacity) {
            Some(end) => end,
            None => return -5,
        };
        for (input_ptr, input_len) in [
            (excluded_ptr as usize, excluded_len),
            (alphabet_ptr as usize, alphabet_len),
        ] {
            if input_len > 0 {
                let input_end = match input_ptr.checked_add(input_len) {
                    Some(end) => end,
                    None => return -5,
                };
                if capacity > 0 && out_start < input_end && input_ptr < out_end {
                    return -5;
                }
            }
        }
        if excluded_len > 0 && alphabet_len > 0 {
            let excluded_start = excluded_ptr as usize;
            let alphabet_start = alphabet_ptr as usize;
            let excluded_end = match excluded_start.checked_add(excluded_len) {
                Some(end) => end,
                None => return -5,
            };
            let alphabet_end = match alphabet_start.checked_add(alphabet_len) {
                Some(end) => end,
                None => return -5,
            };
            if excluded_start < alphabet_end && alphabet_start < excluded_end {
                return -5;
            }
        }

        let excluded_bytes = if excluded_len == 0 {
            &[][..]
        } else {
            slice::from_raw_parts(excluded_ptr, excluded_len)
        };
        let alphabet_bytes = if alphabet_len == 0 {
            &[][..]
        } else {
            slice::from_raw_parts(alphabet_ptr, alphabet_len)
        };
        let excluded = match super::validate_excluded(excluded_bytes) {
            Ok(value) => value,
            Err(_) => return -1,
        };
        let alphabet = match super::validate_excluded(alphabet_bytes) {
            Ok(value) => value,
            Err(_) => return -1,
        };

        let generated = match generate_with_alphabet_and_rng(length, excluded, alphabet, |buffer| {
            // SAFETY: `buffer` is a live stack range in linear memory. The host
            // fills it with cryptographically secure random bytes.
            if random_fill(buffer.as_mut_ptr(), buffer.len() as u32) != 0 {
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
        if generated.len() > capacity {
            return -7;
        }
        if !generated.is_empty() {
            ptr::copy_nonoverlapping(generated.as_ptr(), output_ptr, generated.len());
        }
        generated.len() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::{
        generate_with_alphabet_and_rng, generate_with_rng, validate_excluded, Error,
        DEFAULT_ALPHABET, MAX_LENGTH,
    };

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
    fn rejection_sampling_skips_the_incomplete_random_word_bucket() {
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

    #[test]
    fn generates_requested_number_of_unicode_symbols_from_custom_alphabet() {
        let id = generate_with_alphabet_and_rng(100, "é", "é🙂★λ", |bytes| {
            bytes.fill(0);
            Ok(())
        })
        .unwrap();
        assert_eq!(id.chars().count(), 100);
        assert!(id.chars().all(|symbol| ['🙂', '★', 'λ'].contains(&symbol)));
        assert!(!id.contains('é'));
    }

    #[test]
    fn custom_alphabet_deduplicates_symbols_and_supports_zero_length() {
        let id = generate_with_alphabet_and_rng(50, "", "éé🙂", |bytes| {
            bytes.fill(0);
            Ok(())
        })
        .unwrap();
        assert_eq!(id.chars().count(), 50);
        assert!(id.chars().all(|symbol| symbol == 'é'));
        assert_eq!(
            generate_with_alphabet_and_rng(0, "", "🙂", |_| Ok(())).unwrap(),
            ""
        );
    }
}
