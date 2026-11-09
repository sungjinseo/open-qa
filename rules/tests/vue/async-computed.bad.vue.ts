import { computed } from "vue";

export function useUser(load: () => Promise<string>) {
  const user = computed(async () => await load());
  return { user };
}
