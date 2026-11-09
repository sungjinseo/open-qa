export function merge(target: Record<string, any>, src: Record<string, any>) {
  for (const key in src) {
    target[key] = src[key];
  }
  return target;
}
