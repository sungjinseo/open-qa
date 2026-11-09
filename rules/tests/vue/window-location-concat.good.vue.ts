import { useRouter } from "vue-router";

export function useGo() {
  const router = useRouter();
  return (next: string) => router.push({ path: "/login", query: { next } });
}
