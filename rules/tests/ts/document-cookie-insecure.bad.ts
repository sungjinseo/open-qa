export function remember(sid: string) {
  document.cookie = "sid=" + sid + "; path=/";
}
