package demo;

import java.util.Queue;

// 탐지돼야 하는 경우 3건 (진단가이드 3-2: 탈출 조건 없는 재귀, 같은 타입 인자로 자신을 호출, 무한 반복)
public class Recursion {
    private int a;

    // 기저 조건 없이 자신을 호출한다
    public int factorial(int n) {
        return n * factorial(n - 1);
    }

    public void func(int a) {
        this.a = a;
    }

    // 같은 이름·같은 타입(String) 인자로 자신을 호출한다 (정탐 예)
    public void func(String a) {
        func(a);
    }

    // 빠져나오는 문장이 없는 반복문
    public void drain(Queue<String> queue) {
        while (true) {
            queue.poll();
        }
    }
}
