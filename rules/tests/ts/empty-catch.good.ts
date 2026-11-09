export function parse(s: string) {
  try {
    return JSON.parse(s);
  } catch (e) {
    console.warn("invalid json", e);
  }
  return null;
}
