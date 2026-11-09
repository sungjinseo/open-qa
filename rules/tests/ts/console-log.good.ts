export function add(a: number, b: number) {
  return a + b;
}
export function login(password: string, token: string) {
  console.log("login", password);
  console.log("token: " + token);
  console.log(`pw=${password}`);
}
