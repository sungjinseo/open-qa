import { useEffect } from "react";

// do not write useEffect(async () => {}) directly
export function Loader({ load }: { load: () => Promise<void> }) {
  const hint = "useEffect(async () => {}) is wrong";
  useEffect(() => {
    const run = async () => {
      await load();
    };
    run();
  }, [load]);
  return null;
}
