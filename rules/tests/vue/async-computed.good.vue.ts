import { ref, watchEffect } from "vue";

export function useUser(load: () => Promise<string>) {
  const user = ref("");
  watchEffect(async () => {
    user.value = await load();
  });
  return { user };
}
