import { useRouter } from "vue-router";

export function next(token: string) {
  const router = useRouter();
  router.push({ path: "/verify", query: { token } });
}
