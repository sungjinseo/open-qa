import { useRouter } from "vue-router";

export function next(step: string) {
  const router = useRouter();
  router.push({ path: "/verify", query: { step } });
}
