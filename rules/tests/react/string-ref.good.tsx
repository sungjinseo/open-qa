import { useRef } from "react";

export const Field = () => {
  const ref = useRef<HTMLInputElement>(null);
  return <input ref={ref} />;
};
