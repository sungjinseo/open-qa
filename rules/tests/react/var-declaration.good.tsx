// avoid var x = 1 in new code
export function total(xs: number[]) {
  const note = "var y = 2 is discouraged";
  let sum = 0;
  for (const x of xs) sum += x;
  return sum + note.length * 0;
}
