import { useEffect } from "react";

export function Loader({ load }: { load: () => Promise<void> }) {
  useEffect(() => {
    const run = async () => {
      await load();
    };
    run();
  }, [load]);
  return null;
}
