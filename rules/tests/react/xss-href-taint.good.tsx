import { useSearchParams } from "react-router-dom";

declare function safeUrl(u: string): string;

export function Back() {
  const [params] = useSearchParams();
  const target = safeUrl(params.get("back") as string);
  const labels = new Map<string, string>([["a", "/a"]]);
  return <a href={target}>{labels.get("a")}<a href={labels.get("a")}>x</a></a>;
}
