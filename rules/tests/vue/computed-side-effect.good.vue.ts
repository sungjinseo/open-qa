import { computed } from "vue";

export function useTotal(items: number[]) {
  return computed(() => items.reduce((a, b) => a + b, 0));
}
