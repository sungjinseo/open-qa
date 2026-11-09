import { useEffect } from "react";

export function Clock() {
  useEffect(() => {
    const id = setInterval(() => console.warn("tick"), 1000);
    return () => clearInterval(id);
  }, []);
  return null;
}
