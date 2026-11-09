import { useState } from "react";

export function Box({ open }: { open: boolean }) {
  const [n] = useState(0);
  if (open) {
    return <span>{n}</span>;
  }
  return null;
}
