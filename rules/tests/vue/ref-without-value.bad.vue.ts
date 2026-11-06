import { ref } from "vue";

export function useCounter() {
  let count = ref(0);
  count = 5;
  return { count };
}
