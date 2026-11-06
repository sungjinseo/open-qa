import { ref, onMounted } from "vue";

export function useFocus() {
  const input = ref<HTMLInputElement | null>(null);
  onMounted(() => input.value?.focus());
  return { input };
}
