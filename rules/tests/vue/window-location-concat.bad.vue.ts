export function go(next: string) {
  window.location.href = "/login?next=" + next;
}
