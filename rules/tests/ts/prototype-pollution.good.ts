export function merge(target: Record<string, any>, src: Record<string, any>) {
  for (const key in src) {
    if (key === "__proto__" || key === "constructor") continue;
    target[key] = src[key];
  }
  return target;
}
