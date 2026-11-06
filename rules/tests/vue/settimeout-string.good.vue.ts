export function later(refresh: () => void) {
  setTimeout(() => refresh(), 1000);
}
