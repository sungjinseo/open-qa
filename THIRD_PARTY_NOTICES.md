# 제3자 고지

## kr-secure-coding (MIT)

다음 규칙 파일은 [kr-secure-coding](https://github.com/ace30126/kr-secure-coding)의 JS/TS 규칙(`js-mois.yml`)을 qualgate 규칙 형식(규칙 1개 = 파일 1개, `qualgate_*` 메타데이터, 한국어 메시지)으로 옮기고 일부 패턴을 고쳐 만든 것입니다. 각 파일 맨 위에 `# Adapted from kr-secure-coding (MIT, (c) 2026 ace30126)` 주석이 있습니다.

- `rules/react/dangerous-html.yaml`
- `rules/ts/code-eval.yaml`
- `rules/ts/cookie-insecure.yaml`
- `rules/ts/cors-wildcard.yaml`
- `rules/ts/debug-leftover.yaml`
- `rules/ts/debugger-statement.yaml`
- `rules/ts/empty-catch.yaml`
- `rules/ts/error-leak.yaml`
- `rules/ts/hardcoded-jwt-secret.yaml`
- `rules/ts/header-injection.yaml`
- `rules/ts/insecure-random.yaml`
- `rules/ts/jwt-decode-no-verify.yaml`
- `rules/ts/open-redirect.yaml`
- `rules/ts/os-command.yaml`
- `rules/ts/path-traversal.yaml`
- `rules/ts/public-env-secret.yaml`
- `rules/ts/service-role-in-client.yaml`
- `rules/ts/short-rsa-key.yaml`
- `rules/ts/sql-string-build.yaml`
- `rules/ts/ssrf.yaml`
- `rules/ts/tls-verify-off.yaml`
- `rules/ts/unsafe-deserialize.yaml`
- `rules/ts/unsalted-password-hash.yaml`
- `rules/ts/weak-cipher.yaml`
- `rules/ts/weak-hash.yaml`
- `rules/ts/xss-dom-sink.yaml`
- `rules/ts/xxe.yaml`

행정안전부 보안약점 49개 항목의 번호·이름·CWE 대응표(`docs/mois49.md`의 항목 구성)도 같은 저장소의 `mois49.yml`을 참고했습니다.

수정 안내 문구는 같은 저장소의 `fixes.yml`(항목별 수정 지침)을 참고해 다시 썼습니다.

원본 라이선스 전문입니다.

```
MIT License

Copyright (c) 2026 ace30126

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## 직접 작성한 규칙

`rules/java`, `rules/python`, `rules/go`, `rules/react`, `rules/vue`와 `rules/ts` 중 위 목록에 없는 규칙은 이 프로젝트에서 직접 작성했습니다. `semgrep-rules`, `opengrep-rules`, SonarSource 규칙에서 가져온 내용은 없습니다.
