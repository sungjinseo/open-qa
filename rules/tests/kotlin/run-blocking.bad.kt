import kotlinx.coroutines.*

fun load(): Int = runBlocking {
    delay(10)
    1
}
