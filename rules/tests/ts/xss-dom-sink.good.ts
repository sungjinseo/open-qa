import DOMPurify from "dompurify";
export function show(el: HTMLElement, msg: string) {
  el.textContent = msg;
}
export function showHtml(el: HTMLElement, html: string) {
  el.innerHTML = DOMPurify.sanitize(html);
}
