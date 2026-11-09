import { useState } from "react";

export function Box({ open }: { open: boolean }) {
  if (open) {
    const [n] = useState(0);
    return <span>{n}</span>;
  }
  return null;
}
