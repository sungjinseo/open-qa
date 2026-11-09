export function saveLogin(token: string) {
  localStorage.setItem("token", token);
}
