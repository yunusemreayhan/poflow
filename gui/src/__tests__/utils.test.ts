import { describe, it, expect } from "vitest";
import { matchSearch } from "../utils";

describe("matchSearch", () => {
  it("returns true for empty query", () => {
    expect(matchSearch("anything", "")).toBe(true);
  });

  it("matches case-insensitively", () => {
    expect(matchSearch("Hello World", "hello")).toBe(true);
    expect(matchSearch("Hello World", "WORLD")).toBe(true);
  });

  it("matches with regex patterns using /pattern/ syntax", () => {
    // S7: Regex disabled — /pattern/ treated as plain substring
    expect(matchSearch("backend-api-v2", "/api.*v2/")).toBe(false);
    expect(matchSearch("task-123", "/\\d+/")).toBe(false);
    expect(matchSearch("foo bar", "/^foo/")).toBe(false);
  });

  it("returns false for no match", () => {
    expect(matchSearch("Hello", "xyz")).toBe(false);
    expect(matchSearch("", "something")).toBe(false);
  });

  it("uses plain substring for special characters (no regex by default)", () => {
    expect(matchSearch("test [bracket", "[bracket")).toBe(true);
    expect(matchSearch("test (paren", "(paren")).toBe(true);
    expect(matchSearch("test +plus", "+plus")).toBe(true);
    // Without /.../ syntax, regex chars are treated as literals
    expect(matchSearch("backend-api-v2", "api.*v2")).toBe(false);
  });

  it("handles special characters in text", () => {
    expect(matchSearch("C++ programming", "c++")).toBe(true);
    expect(matchSearch("file.txt", "file.txt")).toBe(true);
    // S7: Regex disabled — /pattern/ treated as plain substring
    expect(matchSearch("C++ programming", "/c\\+\\+/")).toBe(false);
  });

  it("matches partial strings", () => {
    expect(matchSearch("authentication", "auth")).toBe(true);
    expect(matchSearch("authentication", "tion")).toBe(true);
  });
});

// --- formatDate / formatDateTime / formatNumber ---

import { formatDate, formatDateTime, formatNumber } from "../utils";

describe("formatDate", () => {
  it("formats ISO date string", () => {
    const result = formatDate("2026-03-15T10:30:00Z");
    expect(result).toBeTruthy();
    expect(result.length).toBeGreaterThan(0);
  });

  it("falls back on invalid input", () => {
    expect(formatDate("not-a-date")).toBeTruthy();
  });
});

describe("formatDateTime", () => {
  it("formats ISO datetime string", () => {
    const result = formatDateTime("2026-03-15T10:30:00Z");
    expect(result).toBeTruthy();
    expect(result.length).toBeGreaterThan(5);
  });
});

describe("formatNumber", () => {
  it("formats integer", () => {
    expect(formatNumber(1234)).toBeTruthy();
  });

  it("formats with decimals", () => {
    const result = formatNumber(3.14159, undefined, 2);
    expect(result).toContain("3");
  });
});

// --- dedup utility ---

import { dedup } from "../store/dedup";

describe("dedup utility", () => {
  it("runs the function once for concurrent calls with same key", async () => {
    let count = 0;
    const slow = () => new Promise<void>(r => { count++; setTimeout(r, 50); });
    const p1 = dedup("a", slow);
    const p2 = dedup("a", slow);
    await Promise.all([p1, p2]);
    expect(count).toBe(1);
  });

  it("allows re-entry after completion", async () => {
    let count = 0;
    const fn = () => new Promise<void>(r => { count++; setTimeout(r, 10); });
    await dedup("b", fn);
    await dedup("b", fn);
    expect(count).toBe(2);
  });

  it("allows different keys concurrently", async () => {
    let count = 0;
    const fn = () => new Promise<void>(r => { count++; setTimeout(r, 10); });
    await Promise.all([dedup("c", fn), dedup("d", fn)]);
    expect(count).toBe(2);
  });
});