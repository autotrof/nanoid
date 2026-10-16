const DEFAULT_ALPHABET = "_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const MAX_LENGTH = 1_048_576;
const MAX_RANDOM_BYTES = 65_536;

/**
 * Generate a cryptographically secure, URL-safe random ID.
 *
 * @param {number} size Number of ASCII characters to return (default: 21).
 * @param {string} excluded UTF-8 characters to remove from the alphabet.
 * @returns {string}
 */
export function nanoid(size = 21, excluded = "") {
  if (!Number.isSafeInteger(size) || size < 0 || size > MAX_LENGTH) {
    throw new RangeError(`size must be an integer between 0 and ${MAX_LENGTH}`);
  }
  if (typeof excluded !== "string") {
    throw new TypeError("excluded must be a string");
  }

  const excludedCharacters = new Set(excluded);
  let alphabet = "";
  for (const character of DEFAULT_ALPHABET) {
    if (!excludedCharacters.has(character)) alphabet += character;
  }
  if (alphabet.length === 0) {
    throw new RangeError("excluded removes every character from the alphabet");
  }
  if (size === 0) return "";

  const crypto = globalThis.crypto;
  if (typeof crypto?.getRandomValues !== "function") {
    throw new Error("A cryptographic Web Crypto API is required");
  }

  // Reject values outside the largest multiple of alphabet.length below 256.
  // This avoids the modulo bias caused by directly mapping every byte.
  const limit = Math.floor(256 / alphabet.length) * alphabet.length;
  let id = "";
  while (id.length < size) {
    const remaining = size - id.length;
    const count = Math.min(
      MAX_RANDOM_BYTES,
      Math.max(16, Math.ceil((remaining * 256) / limit)),
    );
    const random = crypto.getRandomValues(new Uint8Array(count));
    for (let i = 0; i < random.length && id.length < size; i++) {
      const value = random[i];
      if (value < limit) id += alphabet[value % alphabet.length];
    }
  }
  return id;
}

export default nanoid;
