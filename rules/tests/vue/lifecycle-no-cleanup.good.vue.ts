import { onMounted, onUnmounted } from "vue";

export function useClock() {
  let id = 0;
  onMounted(() => {
    id = window.setInterval(() => console.warn("tick"), 1000);
  });
  onUnmounted(() => clearInterval(id));
}
