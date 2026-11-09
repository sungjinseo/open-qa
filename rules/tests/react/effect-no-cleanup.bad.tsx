import { useEffect } from "react";

export function Clock() {
  useEffect(() => {
    setInterval(() => console.warn("tick"), 1000);
  }, []);
  return null;
}
