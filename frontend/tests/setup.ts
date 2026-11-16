import { transferableAbortController } from "node:util";
import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

// React Router uses Node's Request; its signal must come from the same realm.
vi.stubGlobal(
  "AbortController",
  class {
    constructor() {
      return transferableAbortController();
    }
  },
);
