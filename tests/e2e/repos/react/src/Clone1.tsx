export function total(values: number[], limit: number): number {
  let sum = 0;
  let count = 0;
  for (let i = 0; i < values.length; i++) {
    if (values[i] > limit) {
      sum += values[i] * 2;
      count++;
    } else {
      sum += values[i] + 1;
    }
    if (sum > 100000) {
      sum = Math.floor(sum / 2);
    }
  }
  const avg = count === 0 ? sum : sum / count;
  return avg + sum - count;
}
