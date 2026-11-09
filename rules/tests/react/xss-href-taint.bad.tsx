import { useSearchParams } from "react-router-dom";

export function Back() {
  const [params] = useSearchParams();
  const target = params.get("back") as string;
  return <a href={target}>back</a>;
}
