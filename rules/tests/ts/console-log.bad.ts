export function add(a: number, b: number) {
  console.log("adding", a, b);
  return a + b;
}
export function note(token: string) {
  console.log("token: ", token.length > 0 ? 1 : 0);
}
