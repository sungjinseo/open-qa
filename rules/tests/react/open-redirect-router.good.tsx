import { useNavigate, useSearchParams } from "react-router-dom";

declare function safeRedirectTarget(next: string): string;

export function AfterLogin() {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const next = safeRedirectTarget(params.get("next") as string);
  navigate(next);
  return null;
}
