import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => cleanup());

// jsdom اندازه‌گیری و ResizeObserver ندارد؛ فهرست مجازی‌شده برای رندر ردیف‌ها به اندازهٔ ظرف نیاز دارد.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
(globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
Element.prototype.getBoundingClientRect = function () {
  return { x: 0, y: 0, top: 0, left: 0, right: 320, bottom: 640, width: 320, height: 640, toJSON() {} } as DOMRect;
};
// virtual-core اندازهٔ ظرف اسکرول را از clientWidth/clientHeight می‌خواند
for (const [prop, value] of [["clientHeight", 640], ["clientWidth", 320], ["offsetHeight", 640], ["offsetWidth", 320]] as const) {
  Object.defineProperty(HTMLElement.prototype, prop, { configurable: true, get: () => value });
}
