import { useSearchParams } from "react-router-dom";

declare function safeUrl(u: string): string;

export function Back() {
  const [params] = useSearchParams();
  const target = safeUrl(params.get("back") as string);
  return <a href={target}>back</a>;
}
