import { reactive } from "vue";

export function useForm() {
  const { name, age } = reactive({ name: "a", age: 1 });
  return { name, age };
}
