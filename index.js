const DEFAULT_ALPHABET = "_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const MAX_LENGTH = 1_048_576;
const MAX_ALPHABET_SYMBOLS = 1_048_576;
const UINT32_RANGE = 0x1_0000_0000;
const MAX_RANDOM_WORDS = 16_384;

function assertUnicodeScalars(value, name) {
  for (let i = 0; i < value.length; i++) {
    const code = value.charCodeAt(i);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(i + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        throw new TypeError(`${name} must contain valid Unicode scalar values`);
      }
      i++;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      throw new TypeError(`${name} must contain valid Unicode scalar values`);
    }
  }
}

function createGenerator(excluded, alphabet) {
  if (typeof excluded !== "string") {
    throw new TypeError("excluded must be a string");
  }
  if (typeof alphabet !== "string") {
    throw new TypeError("alphabet must be a string");
  }
  assertUnicodeScalars(excluded, "excluded");
  assertUnicodeScalars(alphabet, "alphabet");

  const excludedSymbols = new Set(excluded);
  const seen = new Set();
  const symbols = [];
  for (const symbol of alphabet) {
    if (!seen.has(symbol)) {
      seen.add(symbol);
      if (!excludedSymbols.has(symbol)) symbols.push(symbol);
      if (seen.size > MAX_ALPHABET_SYMBOLS) {
        throw new RangeError(`alphabet supports at most ${MAX_ALPHABET_SYMBOLS} Unicode symbols`);
      }
    }
  }
  if (symbols.length === 0) {
    throw new RangeError("excluded removes every character from the alphabet");
  }

  const limit = UINT32_RANGE - (UINT32_RANGE % symbols.length);
  return function generate(size = 21) {
    if (!Number.isSafeInteger(size) || size < 0 || size > MAX_LENGTH) {
      throw new RangeError(`size must be an integer between 0 and ${MAX_LENGTH}`);
    }
    if (size === 0) return "";

    const crypto = globalThis.crypto;
    if (typeof crypto?.getRandomValues !== "function") {
      throw new Error("A cryptographic Web Crypto API is required");
    }

    const output = [];
    while (output.length < size) {
      const remaining = size - output.length;
      const count = Math.min(
        MAX_RANDOM_WORDS,
        Math.max(16, Math.ceil((remaining * UINT32_RANGE) / limit)),
      );
      const random = crypto.getRandomValues(new Uint32Array(count));
      for (let i = 0; i < random.length && output.length < size; i++) {
        const value = random[i];
        if (value < limit) output.push(symbols[value % symbols.length]);
      }
    }
    return output.join("");
  };
}

const generateDefault = createGenerator("", DEFAULT_ALPHABET);

/**
 * Generate a cryptographically secure random ID.
 *
 * @param {number} size Number of Unicode symbols to return (default: 21).
 * @param {string} excluded UTF-8 symbols to remove from the alphabet.
 * @param {string} alphabet UTF-8 alphabet; defaults to Nano ID's URL-safe set.
 * @returns {string}
 */
export function nanoid(size = 21, excluded = "", alphabet = DEFAULT_ALPHABET) {
  if (excluded === "" && alphabet === DEFAULT_ALPHABET) return generateDefault(size);
  return createGenerator(excluded, alphabet)(size);
}

/** Create a reusable generator with a custom UTF-8 alphabet. */
export function customAlphabet(alphabet, size = 21, excluded = "") {
  const generate = createGenerator(excluded, alphabet);
  return () => generate(size);
}

export default nanoid;
