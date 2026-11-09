export function login(password: string, a: string) {
  const present = password.length > 0;
  console.error("login attempt", present);
  console.warn("password reset requested");
  console.error("password reset failed");
  console.warn(`token: ${a}`);
}
