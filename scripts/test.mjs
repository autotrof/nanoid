import assert from "node:assert/strict";
import test from "node:test";
import { nanoid } from "../index.js";

test("generates URL-safe IDs with the requested size", () => {
  assert.match(nanoid(), /^[A-Za-z0-9_-]{21}$/);
  assert.match(nanoid(100), /^[A-Za-z0-9_-]{100}$/);
  assert.equal(nanoid(0), "");
});

test("excludes ASCII symbols and accepts UTF-8 exclusions", () => {
  const id = nanoid(100, "_-abcé🦀");
  assert.match(id, /^[A-Za-z0-9_-]{100}$/);
  assert.doesNotMatch(id, /[_\-abc]/);
});

test("rejects invalid arguments and an empty alphabet", () => {
  assert.throws(() => nanoid(-1), RangeError);
  assert.throws(() => nanoid(1.5), RangeError);
  assert.throws(() => nanoid(1, null), TypeError);
  assert.throws(() => nanoid(1, "_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"), RangeError);
});
