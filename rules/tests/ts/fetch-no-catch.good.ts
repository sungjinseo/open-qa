export function load(setData: (d: unknown) => void) {
  fetch("/api/items")
    .then((r) => r.json())
    .then(setData)
    .catch((e) => console.warn("load failed", e));
}
