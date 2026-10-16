# Nano ID for WebAssembly

Small Rust Nano ID generator shipped as one portable raw `.wasm` module. The
host supplies cryptographically secure random bytes, so the same binary works
in browsers, Cloudflare Workers, and Node.js without WASI or JavaScript glue.

## Download

Download [`nanoid.wasm` from the latest release](https://github.com/autotrof/nanoid/releases/latest/download/nanoid.wasm).
The module exports linear memory, `alloc_buffer(length)`,
`free_buffer(pointer, length)`, and `generate(outputPtr, length, excludedPtr,
excludedLength)`. It imports `env.random_fill(pointer, length) -> i32`. The host
must fill the provided linear-memory range with cryptographically secure random
bytes and return `0` on success. `generate` requests bytes in batches of 64.

## ABI

| Item | Contract |
| --- | --- |
| Import | `env.random_fill(i32, i32) -> i32`; fill the linear-memory range with CSPRNG bytes and return `0` on success |
| Exports | `memory`, `alloc_buffer(i32) -> i32`, `free_buffer(i32, i32)`, and `generate(i32, i32, i32, i32) -> i32` |
| Arguments | output pointer, output length in bytes, UTF-8 exclusions pointer, exclusion byte length |
| Success | `0`; exactly `length` ASCII bytes are written at `outputPtr` |
| Errors | `-1` invalid UTF-8; `-2` output length over 1,048,576; `-3` exclusions remove all symbols; `-4` random import failed; `-5` null, overlapping, or overflowing ranges; `-6` exclusions exceed 1,048,576 bytes |
| Memory | Call `alloc_buffer` for host input/output, then `free_buffer` with the original pointer and length. Allocator-owned buffers remain valid while Rust allocates internally. Input/output ranges must not overlap. |
| Defaults | Alphabet is `_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ` (64 symbols) |

Exclusions are UTF-8 text interpreted as Unicode scalar values. Excluding `a`
removes ASCII `a`; excluding `é` or `🦀` is valid and leaves the default ASCII
alphabet unchanged. Invalid UTF-8 is rejected before output is written. The
module rejects a set that removes every default alphabet symbol. Random bytes
are mapped with rejection sampling, so modulo reduction does not introduce
bias. Use only a CSPRNG import for production IDs.

## Browser

Save `nanoid.wasm` beside this module, or replace the URL with the release URL.
Run from a local web server because browsers do not fetch `file://` URLs.

```js
async function main() {
  let instance;
  ({ instance } = await WebAssembly.instantiateStreaming(fetch("./nanoid.wasm"), {
    env: {
      random_fill(pointer, length) {
        try {
          crypto.getRandomValues(new Uint8Array(instance.exports.memory.buffer, pointer >>> 0, length >>> 0));
          return 0;
        } catch {
          return 1;
        }
      },
    },
  }));

  function nanoid(length = 21, excluded = "") {
    const { memory, alloc_buffer, free_buffer, generate } = instance.exports;
    const excludedBytes = new TextEncoder().encode(excluded);
    const outputPtr = alloc_buffer(length) >>> 0;
    const excludedPtr = alloc_buffer(excludedBytes.length) >>> 0;
    if (!outputPtr || !excludedPtr) throw new Error("Wasm buffer allocation failed");
    try {
      new Uint8Array(memory.buffer, excludedPtr, excludedBytes.length).set(excludedBytes);
      const status = generate(outputPtr, length, excludedPtr, excludedBytes.length);
      if (status !== 0) throw new Error(`nanoid wasm error ${status}`);
      return new TextDecoder().decode(new Uint8Array(memory.buffer, outputPtr, length));
    } finally {
      free_buffer(excludedPtr, excludedBytes.length);
      free_buffer(outputPtr, length);
    }
  }

  console.log(nanoid(21, "-_"));
}

main();
```

## Cloudflare Workers

Bundle the downloaded binary as `nanoid.wasm` and include it in the Worker
bundle. `crypto` is the Workers Web Crypto API.

```js
import wasmModule from "./nanoid.wasm";

export default {
  async fetch() {
    let instance;
    instance = await WebAssembly.instantiate(wasmModule, {
      env: {
        random_fill(pointer, length) {
          try {
            crypto.getRandomValues(new Uint8Array(instance.exports.memory.buffer, pointer >>> 0, length >>> 0));
            return 0;
          } catch {
            return 1;
          }
        },
      },
    });
    const { memory, alloc_buffer, free_buffer, generate } = instance.exports;
    const excludedBytes = new TextEncoder().encode("-_");
    const outputPtr = alloc_buffer(21) >>> 0;
    const excludedPtr = alloc_buffer(excludedBytes.length) >>> 0;
    if (!outputPtr || !excludedPtr) return new Response("Wasm buffer allocation failed", { status: 500 });
    try {
      new Uint8Array(memory.buffer, excludedPtr, excludedBytes.length).set(excludedBytes);
      const status = generate(outputPtr, 21, excludedPtr, excludedBytes.length);
      if (status !== 0) return new Response(`nanoid wasm error ${status}`, { status: 500 });
      const id = new TextDecoder().decode(new Uint8Array(memory.buffer, outputPtr, 21));
      return Response.json({ id });
    } finally {
      free_buffer(excludedPtr, excludedBytes.length);
      free_buffer(outputPtr, 21);
    }
  },
};
```

## Node.js

This example uses CommonJS and Node's Web Crypto implementation. Node 18 or
newer is recommended.

```js
const fs = require("node:fs");
const { webcrypto } = require("node:crypto");

async function main() {
  const { instance } = await WebAssembly.instantiate(
    fs.readFileSync("./nanoid.wasm"),
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
  );

  function nanoid(length = 21, excluded = "") {
    const { memory, alloc_buffer, free_buffer, generate } = instance.exports;
    const excludedBytes = Buffer.from(excluded, "utf8");
    const outputPtr = alloc_buffer(length) >>> 0;
    const excludedPtr = alloc_buffer(excludedBytes.length) >>> 0;
    if (!outputPtr || !excludedPtr) throw new Error("Wasm buffer allocation failed");
    try {
      new Uint8Array(memory.buffer, excludedPtr, excludedBytes.length).set(excludedBytes);
      const status = generate(outputPtr, length, excludedPtr, excludedBytes.length);
      if (status !== 0) throw new Error(`nanoid wasm error ${status}`);
      return Buffer.from(memory.buffer, outputPtr, length).toString("ascii");
    } finally {
      free_buffer(excludedPtr, excludedBytes.length);
      free_buffer(outputPtr, length);
    }
  }

  console.log(nanoid(21, "-_"));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
```

Each wrapper uses the module's allocator for host buffers. This protects those
buffers while the Rust generator allocates its temporary alphabet and output.
The generated output is ASCII, and the requested length is measured in bytes
(equivalently, characters).

## Rust API

The Rust crate exposes `generate_with_rng` for applications that already have a
secure random source:

```rust
use nanoid::generate_with_rng;

let id = generate_with_rng(21, "-_", |bytes| {
    getrandom::getrandom(bytes).map_err(|_| ())
})?;
```

The example uses the application's `getrandom` dependency as the entropy
provider. The library deliberately does not choose or weaken the caller's
randomness source.

## Build and test

```sh
cargo test
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

The release build is at
`target/wasm32-unknown-unknown/release/nanoid.wasm`. Push a `v*` tag to run CI,
which tests the crate and attaches `nanoid.wasm` as a directly downloadable
GitHub Release asset.
