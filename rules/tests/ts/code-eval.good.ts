export function run(expr: string) {
  return JSON.parse(expr);
}
export function later(name: string) {
  setTimeout(() => console.warn(name), 100);
}
