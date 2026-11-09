import { useRoute, useRouter } from "vue-router";

export function afterLogin() {
  const route = useRoute();
  const router = useRouter();
  router.push(route.query.next as string);
}
