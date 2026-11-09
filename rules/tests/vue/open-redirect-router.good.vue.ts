import { useRoute, useRouter } from "vue-router";

declare function safeRedirectTarget(next: string): string;

export function afterLogin() {
  const route = useRoute();
  const router = useRouter();
  router.push(safeRedirectTarget(route.query.next as string));
}
