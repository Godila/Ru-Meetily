// Global test setup. Runs once before every test file.
//
// Adds jest-dom matchers (toBeInTheDocument, toHaveClass, ...) so React
// component assertions read cleanly. Kept minimal on purpose: per-test setup
// belongs in the test file, not here.
import '@testing-library/jest-dom/vitest';

// jsdom does not implement matchMedia; some components (e.g. radix-ui) call it
// at import time. Provide a no-op stub so importing them doesn't throw.
if (!window.matchMedia) {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }),
  });
}

// ResizeObserver is used by a few UI libs (cmdk, radix scroll-area) and is
// absent in jsdom. Stub it so imports don't crash.
if (!('ResizeObserver' in globalThis)) {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}
