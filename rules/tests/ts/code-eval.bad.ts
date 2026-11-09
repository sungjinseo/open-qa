export function run(expr: string) {
  return eval(expr);
}
export function later(name: string) {
  setTimeout("greet('" + name + "')", 100);
}
