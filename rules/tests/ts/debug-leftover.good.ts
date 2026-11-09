export function login(password: string) {
  const present = password.length > 0;
  console.log("login attempt", present);
}
