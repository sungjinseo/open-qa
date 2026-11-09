export function total(items: number[]) {
  return items.reduce((a, b) => a + b, 0);
}
