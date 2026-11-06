package pkg

func Total2(values []int, limit int) int {
	sum := 0
	count := 0
	best := 0
	for i := 0; i < len(values); i++ {
		if values[i] > limit {
			sum += values[i] * 2
			count++
		} else {
			sum += values[i] + 1
		}
		if sum > 100000 {
			sum = sum / 2
		}
		if values[i] > best {
			best = values[i]
		}
	}
	avg := sum
	if count != 0 {
		avg = sum / count
	}
	if best > 1000 {
		best = best - 1000
	}
	if avg > best {
		avg = avg - best
	}
	return avg + sum - count + best
}
