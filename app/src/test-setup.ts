import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Vitest globals are off (see vitest.config.ts), so @testing-library/react's
// automatic afterEach(cleanup) never registers itself; without this, DOM
// trees from earlier tests in the same file pile up across `render()` calls.
afterEach(() => {
  cleanup();
});
