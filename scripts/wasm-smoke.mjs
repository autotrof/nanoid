import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { webcrypto } from "node:crypto";

let instance;
({ instance } = await WebAssembly.instantiate(
  await readFile(new URL("../target/wasm32-unknown-unknown/release/nanoid.wasm", import.meta.url)),
  {
    env: {
      random_fill(pointer, length) {
        try {
          webcrypto.getRandomValues(
            new Uint8Array(instance.exports.memory.buffer, pointer >>> 0, length >>> 0),
          );
          return 0;
        } catch {
          return 1;
        }
      },
    },
  },
));

const { memory, alloc_buffer, free_buffer, generate } = instance.exports;
function allocate(bytes) {
  const pointer = alloc_buffer(bytes) >>> 0;
  assert.notEqual(pointer, 0, "Wasm allocator should return a buffer");
  return pointer;
}

const length = 8192;
const excluded = Buffer.from("🦀".repeat(20_000));
const outputPtr = allocate(length);
const excludedPtr = allocate(excluded.length);
new Uint8Array(memory.buffer, excludedPtr, excluded.length).set(excluded);
assert.equal(generate(outputPtr, length, excludedPtr, excluded.length), 0);
const id = Buffer.from(memory.buffer, outputPtr, length).toString("ascii");
assert.equal(id.length, length);
assert.match(id, /^[\w-]+$/);
free_buffer(excludedPtr, excluded.length);
free_buffer(outputPtr, length);

const badUtf8Ptr = allocate(1);
new Uint8Array(memory.buffer, badUtf8Ptr, 1)[0] = 0xff;
const emptyOutputPtr = allocate(1);
assert.equal(generate(emptyOutputPtr, 1, badUtf8Ptr, 1), -1);
free_buffer(badUtf8Ptr, 1);
free_buffer(emptyOutputPtr, 1);

const overlapPtr = allocate(8);
assert.equal(generate(overlapPtr, 4, overlapPtr, 4), -5);
free_buffer(overlapPtr, 8);

assert.equal(generate(0, 1_048_577, 0, 0), -2);
assert.equal(generate(0, 1, 0, 1_048_577), -6);

const allSymbols = Buffer.from("_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ");
const allSymbolsPtr = allocate(allSymbols.length);
const allSymbolsOutputPtr = allocate(1);
new Uint8Array(memory.buffer, allSymbolsPtr, allSymbols.length).set(allSymbols);
assert.equal(generate(allSymbolsOutputPtr, 1, allSymbolsPtr, allSymbols.length), -3);
free_buffer(allSymbolsPtr, allSymbols.length);
free_buffer(allSymbolsOutputPtr, 1);

const alphabet = Buffer.from("é🙂★λ");
const excludedSymbols = Buffer.from("é");
const utf8OutputPtr = allocate(100 * 4);
const alphabetPtr = allocate(alphabet.length);
const excludedSymbolsPtr = allocate(excludedSymbols.length);
new Uint8Array(memory.buffer, alphabetPtr, alphabet.length).set(alphabet);
new Uint8Array(memory.buffer, excludedSymbolsPtr, excludedSymbols.length).set(excludedSymbols);
const written = instance.exports.generate_with_alphabet(
  utf8OutputPtr, 100 * 4, 100,
  excludedSymbolsPtr, excludedSymbols.length,
  alphabetPtr, alphabet.length,
);
assert.ok(written > 0);
const unicodeId = new TextDecoder().decode(new Uint8Array(memory.buffer, utf8OutputPtr, written));
assert.equal([...unicodeId].length, 100);
assert.ok([...unicodeId].every((symbol) => ["🙂", "★", "λ"].includes(symbol)));
assert.equal(instance.exports.generate_with_alphabet(
  utf8OutputPtr, 1, 100,
  excludedSymbolsPtr, excludedSymbols.length,
  alphabetPtr, alphabet.length,
), -7);
free_buffer(excludedSymbolsPtr, excludedSymbols.length);
free_buffer(alphabetPtr, alphabet.length);
free_buffer(utf8OutputPtr, 100 * 4);

console.log("Wasm smoke test passed (ASCII compatibility, UTF-8 alphabet, ABI errors).");
