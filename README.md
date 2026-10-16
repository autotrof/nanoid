# Nano ID for JavaScript

Small, dependency-free JavaScript implementation of the Nano ID generator in
this repository. It uses the host's Web Crypto API and returns URL-safe ASCII
IDs. The `js` branch contains only the library and its documentation.

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
import { nanoid } from "@autotrof/nanoid";

const id = nanoid();       // 21 characters
const shortId = nanoid(12);
const customId = nanoid(16, "-_0O"); // these characters never appear
```

The package is native ESM and has no runtime or development dependencies. It
works in browsers and Cloudflare Workers that provide Web Crypto, and in Node.js
20 or newer. Use a bundler or a local file server when importing it in a
browser.

## API

`nanoid(size = 21, excluded = "")`

- `size`: integer from 0 through 1,048,576; output length is measured in ASCII
  characters.
- `excluded`: UTF-8 string of characters to remove from the default alphabet
  `_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ`.
- Returns a string. Throws `RangeError` for invalid sizes or if every alphabet
  symbol is excluded, and `TypeError` if `excluded` is not a string.

Each Unicode character in `excluded` is interpreted as a code point. The
alphabet is ASCII, so non-ASCII exclusions are valid but have no effect.
Character selection uses rejection sampling to avoid modulo bias. The ID is
cryptographically random, but callers must still choose a sufficient length
for their expected number of generated IDs.
