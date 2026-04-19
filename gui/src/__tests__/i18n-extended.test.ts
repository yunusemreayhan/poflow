import { describe, it, expect, beforeEach } from "vitest";
import { useI18n, interpolate, plural } from "../i18n";

beforeEach(() => {
  useI18n.getState().setLocale("en");
});

describe("interpolate", () => {
  it("replaces single variable", () => {
    expect(interpolate("Hello {name}", { name: "World" })).toBe("Hello World");
  });

  it("replaces multiple variables", () => {
    expect(interpolate("{a} and {b}", { a: "X", b: "Y" })).toBe("X and Y");
  });

  it("replaces same variable multiple times", () => {
    expect(interpolate("{x} + {x}", { x: "1" })).toBe("1 + 1");
  });

  it("preserves unknown variables", () => {
    expect(interpolate("Hello {unknown}", {})).toBe("Hello {unknown}");
  });

  it("handles numeric values", () => {
    expect(interpolate("{count} items", { count: 42 })).toBe("42 items");
  });

  it("handles empty string value", () => {
    expect(interpolate("Hello {name}", { name: "" })).toBe("Hello ");
  });

  it("handles no variables in template", () => {
    expect(interpolate("No vars here", { x: "y" })).toBe("No vars here");
  });

  it("handles empty template", () => {
    expect(interpolate("", { x: "y" })).toBe("");
  });
});

describe("plural", () => {
  it("returns singular for 1", () => {
    expect(plural(1, "item", "items")).toBe("item");
  });

  it("returns plural for 0", () => {
    expect(plural(0, "item", "items")).toBe("items");
  });

  it("returns plural for > 1", () => {
    expect(plural(2, "task", "tasks")).toBe("tasks");
    expect(plural(100, "session", "sessions")).toBe("sessions");
  });

  it("returns plural for negative", () => {
    expect(plural(-1, "item", "items")).toBe("items");
  });
});

describe("locale switching", () => {
  it("defaults to English", () => {
    expect(useI18n.getState().locale).toBe("en");
  });

  it("switches to Turkish", () => {
    useI18n.getState().setLocale("tr");
    expect(useI18n.getState().locale).toBe("tr");
    expect(useI18n.getState().t.logout).toBe("Çıkış");
  });

  it("switches back to English", () => {
    useI18n.getState().setLocale("tr");
    useI18n.getState().setLocale("en");
    expect(useI18n.getState().t.logout).toBe("Logout");
  });

  it("unknown locale falls back to English", () => {
    useI18n.getState().setLocale("xx");
    expect(useI18n.getState().t.appName).toBe("Poflow");
  });

  it("availableLocales includes en and tr", () => {
    const locales = useI18n.getState().availableLocales();
    expect(locales).toContain("en");
    expect(locales).toContain("tr");
  });
});

describe("locale completeness", () => {
  it("English has no empty values", () => {
    useI18n.getState().setLocale("en");
    const { t } = useI18n.getState();
    for (const [key, value] of Object.entries(t)) {
      expect(value, `Empty English key: ${key}`).not.toBe("");
    }
  });

  it("Turkish has no empty values", () => {
    useI18n.getState().setLocale("tr");
    const { t } = useI18n.getState();
    for (const [key, value] of Object.entries(t)) {
      expect(value, `Empty Turkish key: ${key}`).not.toBe("");
    }
  });

  it("English and Turkish have same keys", () => {
    useI18n.getState().setLocale("en");
    const enKeys = Object.keys(useI18n.getState().t).sort();
    useI18n.getState().setLocale("tr");
    const trKeys = Object.keys(useI18n.getState().t).sort();
    expect(enKeys).toEqual(trKeys);
  });

  it("has 80+ locale keys", () => {
    const keys = Object.keys(useI18n.getState().t);
    expect(keys.length).toBeGreaterThanOrEqual(80);
  });
});
