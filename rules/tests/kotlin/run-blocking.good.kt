import kotlinx.coroutines.*

suspend fun load(): Int {
    delay(10)
    return 1
}
