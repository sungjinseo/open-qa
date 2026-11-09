import { useNavigate, useSearchParams } from "react-router-dom";

export function AfterLogin() {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const next = params.get("next") as string;
  navigate(next);
  return null;
}
