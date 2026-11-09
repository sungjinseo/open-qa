export function banner(msg: string) {
  const h = document.createElement("h1");
  h.textContent = msg;
  document.body.appendChild(h);
}
