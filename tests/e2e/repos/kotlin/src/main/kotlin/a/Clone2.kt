package a

class Clone2 {
    fun total(values: IntArray, limit: Int): Int {
        var sum = 0
        var count = 0
        for (i in values.indices) {
            if (values[i] > limit) {
                sum += values[i] * 2
                count++
            } else {
                sum += values[i] + 1
            }
            if (sum > 100000) {
                sum = sum / 2
            }
        }
        val avg = if (count == 0) sum else sum / count
        return avg + sum - count
    }

    fun scale(values: IntArray, factor: Int): List<Int> {
        val out = mutableListOf<Int>()
        var total = 0
        for (i in values.indices) {
            var item = values[i] * factor
            if (item > 1000) {
                item = item - 1000
            }
            total += item
            out.add(item)
        }
        if (total == 0) {
            return out
        }
        return out.map { it * 100 / total }
    }
}
