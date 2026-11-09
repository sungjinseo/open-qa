import { useRoute } from "vue-router";

const PAGES: Record<string, () => Promise<unknown>> = {
  home: () => import("./pages/home.vue"),
  about: () => import("./pages/about.vue"),
};

export function loadPage() {
  const route = useRoute();
  return PAGES[String(route.params.name)]?.();
}
