import { useEffect } from "react";

export function Loader({ load }: { load: () => Promise<void> }) {
  useEffect(async () => {
    await load();
  }, [load]);
  return null;
}
