package a;

class Clone2 {
    int total(int[] values, int limit) {
        int sum = 0;
        int count = 0;
        for (int i = 0; i < values.length; i++) {
            if (values[i] > limit) {
                sum += values[i] * 2;
                count++;
            } else {
                sum += values[i] + 1;
            }
            if (sum > 100000) {
                sum = sum / 2;
            }
        }
        int avg = count == 0 ? sum : sum / count;
        return avg + sum - count;
    }

    int scale(int[] values, int factor) {
        int total = 0;
        int best = 0;
        for (int i = 0; i < values.length; i++) {
            int item = values[i] * factor;
            if (item > 1000) {
                item = item - 1000;
            }
            total += item;
            if (item > best) {
                best = item;
            }
        }
        if (total == 0) {
            return 0;
        }
        return best * 100 / total;
    }
}
