# Nano ID for JavaScript

Small, dependency-free JavaScript Nano ID generator. It uses the host's Web
Crypto API and supports custom alphabets with Unicode symbols. The `js` branch
contains only the library and its documentation.

## Install from this repository

Add the branch as a dependency in your project's `package.json`:

```json
{
  "dependencies": {
    "@autotrof/nanoid": "github:autotrof/nanoid#js"
  }
}
```

Then run `npm install` and use it:

```js
import { customAlphabet, nanoid } from "@autotrof/nanoid";

const id = nanoid();       // 21 characters
const shortId = nanoid(12);
const customId = nanoid(16, "-_0O"); // these characters never appear
const unicodeId = nanoid(12, "", "abcé★🙂");
const makeSymbolId = customAlphabet("abcé★🙂", 12, "é");
const anotherId = makeSymbolId();
```

The package is native ESM and has no runtime or development dependencies. It
works in browsers and Cloudflare Workers that provide Web Crypto, and in Node.js
20 or newer. Use a bundler or a local file server when importing it in a
browser.

## API

`nanoid(size = 21, excluded = "", alphabet = defaultAlphabet)`

- `size`: integer from 0 through 1,048,576; output length counts Unicode scalar
  values, not UTF-8 bytes or grapheme clusters.
- `excluded`: UTF-8 string of symbols to remove from the alphabet.
- `alphabet`: UTF-8 alphabet. Defaults to
  `_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ`.
- Repeated alphabet symbols are deduplicated. Each remaining Unicode symbol
  has equal probability.
- `customAlphabet(alphabet, size = 21, excluded = "")` returns a reusable
  zero-argument generator.
- Throws `RangeError` for invalid sizes, an empty alphabet, or too many
  alphabet symbols; throws `TypeError` for non-string or malformed Unicode
  inputs.

The alphabet and exclusion list may contain letters, punctuation, emoji, and
other UTF-8 symbols. Character selection uses rejection sampling to avoid
modulo bias. The ID is cryptographically random, but callers must still choose
a sufficient length for their expected number of generated IDs.
