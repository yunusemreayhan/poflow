import { describe, it, expect } from "vitest";
import { ErrorBoundary } from "../components/ErrorBoundary";
import React from "react";
import ReactDOMServer from "react-dom/server";

describe("ErrorBoundary", () => {
  it("renders children when no error", () => {
    const html = ReactDOMServer.renderToString(
      React.createElement(ErrorBoundary, null,
        React.createElement("div", null, "Hello World")
      )
    );
    expect(html).toContain("Hello World");
  });

  it("has error state handling", () => {
    // ErrorBoundary is a class component with getDerivedStateFromError
    expect(ErrorBoundary.getDerivedStateFromError).toBeDefined();
    const result = ErrorBoundary.getDerivedStateFromError(new Error("test"));
    expect(result).toEqual({ hasError: true, error: expect.any(Error) });
  });
});
