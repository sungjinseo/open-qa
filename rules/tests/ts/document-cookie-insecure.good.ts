export function remember(sid: string) {
  document.cookie = "sid=" + sid + "; path=/; Secure; SameSite=Lax";
}
