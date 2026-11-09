export function build(body: string) {
  return new Function("a", "b", body);
}
