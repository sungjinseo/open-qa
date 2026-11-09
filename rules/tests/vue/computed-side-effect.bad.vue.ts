import { computed, ref } from "vue";

export function useTotal(items: number[]) {
  const count = ref(0);
  return computed(() => {
    count.value = items.length;
    return items.reduce((a, b) => a + b, 0);
  });
}
