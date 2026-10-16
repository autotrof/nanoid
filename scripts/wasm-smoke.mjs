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

console.log("Wasm smoke test passed (8192-byte ID, large UTF-8 exclusions, ABI errors).");
