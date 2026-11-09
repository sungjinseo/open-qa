import { onMounted } from "vue";

export function useClock() {
  onMounted(() => {
    setInterval(() => console.warn("tick"), 1000);
  });
}
