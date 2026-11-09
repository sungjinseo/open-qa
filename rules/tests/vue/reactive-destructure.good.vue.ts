import { reactive, toRefs } from "vue";

export function useForm() {
  const state = reactive({ name: "a", age: 1 });
  const { name, age } = toRefs(state);
  return { name, age };
}
