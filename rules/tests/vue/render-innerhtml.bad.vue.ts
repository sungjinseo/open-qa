import { h } from "vue";

export function render(html: string) {
  return h("div", { innerHTML: html });
}
