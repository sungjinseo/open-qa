import { ref, watchEffect } from "vue";

// computed(async () => ...) does not work
export function useUser(load: () => Promise<string>) {
  const hint = "computed(async () => 1) is wrong";
  const user = ref("");
  watchEffect(async () => {
    user.value = await load();
  });
  return { user };
}
