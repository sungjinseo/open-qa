package demo;

import java.util.List;
import java.util.Queue;

// 탐지되면 안 되는 경우 (진단가이드 3-2 안전한 코드와 오탐 예)
public class Recursion {
    private int a;

    // 기저 조건(Base case)이 있다
    public int factorial(int n) {
        if (n <= 1) {
            return 1;
        }
        return n * factorial(n - 1);
    }

    public void func(int a) {
        this.a = a;
    }

    // 다른 오버로드(int)를 호출하므로 재귀가 아니다 (오탐 예)
    public void func(String a) {
        func(Integer.valueOf(a).intValue());
    }

    // 자식이 없으면 반복문을 돌지 않아 끝난다
    public int depth(Node node) {
        int max = 0;
        for (Node child : node.children) {
            max = Math.max(max, depth(child));
        }
        return max + 1;
    }

    // 꺼낼 것이 없으면 빠져나온다
    public int drain(Queue<String> queue) {
        int n = 0;
        while (true) {
            if (queue.poll() == null) {
                break;
            }
            n++;
        }
        return n;
    }

    static final class Node {
        private final List<Node> children;

        Node(List<Node> children) {
            this.children = children;
        }
    }
}
