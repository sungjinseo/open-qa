import Vue from "vue";

export function build(userTemplate: string) {
  return Vue.compile(userTemplate);
}
