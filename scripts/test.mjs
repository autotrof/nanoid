import assert from "node:assert/strict";
import test from "node:test";
import { customAlphabet, nanoid } from "../index.js";

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

test("generates custom UTF-8 symbols and counts Unicode code points", () => {
  const id = nanoid(100, "é", "é🙂★λ");
  assert.equal([...id].length, 100);
  assert.ok([...id].every((symbol) => ["🙂", "★", "λ"].includes(symbol)));

  const makeId = customAlphabet("ab🙂🙂", 50, "a");
  const customId = makeId();
  assert.equal([...customId].length, 50);
  assert.ok([...customId].every((symbol) => symbol === "b" || symbol === "🙂"));
});

test("rejects invalid arguments and an empty alphabet", () => {
  assert.throws(() => nanoid(-1), RangeError);
  assert.throws(() => nanoid(1.5), RangeError);
  assert.throws(() => nanoid(1, null), TypeError);
  assert.throws(() => nanoid(1, "_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"), RangeError);
  assert.throws(() => nanoid(1, "", ""), RangeError);
  assert.throws(() => nanoid(1, "", "\ud800"), TypeError);
});
