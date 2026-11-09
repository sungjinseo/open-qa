import { ref } from "vue";

export function useCounter() {
  const count = ref(0);
  count.value = 5;
  return { count };
}
