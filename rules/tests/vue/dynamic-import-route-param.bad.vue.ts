import { useRoute } from "vue-router";

export function loadPage() {
  const route = useRoute();
  return import(`./pages/${route.params.name}.vue`);
}
