import { useEffect, useState } from "react";

export function Counter() {
  const [n, setN] = useState(0);
  useEffect(() => {
    setN(n + 1);
  });
  return <span>{n}</span>;
}
