import kotlinx.coroutines.*

class Worker(private val scope: CoroutineScope) {
    fun start() {
        scope.launch {
            delay(1000)
        }
    }
}
