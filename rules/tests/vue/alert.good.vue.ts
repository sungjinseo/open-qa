import { ref } from "vue";

export function useWarn() {
  const message = ref("");
  const warn = (msg: string) => {
    message.value = msg;
  };
  return { message, warn };
}
