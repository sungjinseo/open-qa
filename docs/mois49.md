# 행정안전부 SW 보안약점 49 점검 범위

행정안전부 「소프트웨어 보안약점 진단가이드」(2021) 구현단계 보안약점 49개 항목과 바이브코딩 SaaS에서 사고가 잦은 확장 항목(VC-01..04)이 기본 규칙에 어떻게 대응하는지 정리한 표입니다. 항목 이름과 순서는 2021판 목차 기준 정리본이며 원문 PDF와의 최종 대조는 남아 있습니다.

읽는 법

- 규칙 ID: 그 언어 폴더(`rules/<언어>/`)에서 해당 항목을 점검하는 규칙입니다.
- `부분:` 표시는 항목 전체가 아니라 흔한 API·패턴 일부만 자동으로 찾는다는 뜻입니다. 나머지는 사람이 확인해야 합니다.
- `해당 없음`: 그 언어·런타임 특성상 약점이 성립하지 않는 경우이며 이유를 적었습니다.
- `수동 확인`: 코드만으로 판단할 수 없는 항목입니다. `수동 확인(규칙 미작성)`은 자동 점검이 가능할 수 있으나 이번 규칙 묶음에 넣지 않은 항목입니다.
- `ts` 폴더 규칙은 프레임워크와 무관하게 `.js`, `.ts`, `.jsx`, `.tsx`와 Vue `<script>`에 모두 적용됩니다. react·vue 열의 `ts 규칙이 적용됨`은 이를 뜻하며, 중복 보고를 피하려고 react·vue 폴더에는 같은 규칙을 두지 않았습니다.
- Vue는 `<script>`와 `<script setup>`만 분석하므로 템플릿의 `v-html`, 템플릿 표현식은 볼 수 없습니다.
- 규칙은 한 파일 안의 패턴과 파일 내부 데이터 흐름(taint)만 봅니다. 파일 간 흐름, MyBatis XML의 `${}`, 설정 파일 값은 점검하지 못합니다.
- `†` 표시는 규칙의 CWE가 mois49.yml의 해당 항목 CWE 목록에 없는 **확장 매핑**입니다. 가장 가까운 항목에 연결했으며 공식 매핑이 아닙니다.
- `확장` 항목(VC)은 행안부 49개에 없는 추가 점검이며 메시지 접두어는 `[행안부 확장 VC-xx ...]`입니다.

| ID | 항목 | java | python | ts | react | vue | go |
|---|---|---|---|---|---|---|---|
| IV-01 | SQL 삽입 | `java.sql-concat`<br>`java.sql-update-concat` | `python.sql-format`<br>`python.sql-fstring` | `ts.sql-string-build` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.sql-concat`<br>`go.sql-sprintf` |
| IV-02 | 코드 삽입 | `java.code-injection-script` | `python.eval`<br>`python.exec`<br>`python.flask-render-template-string` | `ts.code-eval` | ts 규칙이 적용됨 | `vue.runtime-template-compile`<br>+ ts 규칙 적용 | `go.template-parse-nonliteral` |
| IV-03 | 경로 조작 및 자원 삽입 | `java.path-traversal` | `python.path-traversal` | `ts.path-traversal` | ts 규칙이 적용됨 | `vue.dynamic-import-route-param`†<br>+ ts 규칙 적용 | `go.path-traversal` |
| IV-04 | 크로스사이트 스크립트 | `java.xss-servlet-writer` | `python.jinja-autoescape-off`<br>`python.xss-unescaped-output` | `ts.xss-dom-sink` | `react.dangerous-html`<br>`react.href-javascript`<br>`react.iframe-no-sandbox`†<br>`react.xss-href-taint`<br>+ ts 규칙 적용 | `vue.render-innerhtml` (script 영역만(템플릿 v-html 불가))<br>+ ts 규칙 적용 | `go.template-html-unescaped`<br>`go.xss-response-write` |
| IV-05 | 운영체제 명령어 삽입 | `java.runtime-exec-concat` | `python.os-system`<br>`python.subprocess-shell` | `ts.os-command` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.exec-shell` |
| IV-06 | 위험한 형식 파일 업로드 | 부분: `java.upload-original-filename` | 부분: `python.upload-unsafe-filename` | 부분: `ts.upload-no-filter` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 부분: `go.upload-original-filename` |
| IV-07 | 신뢰되지 않은 URL주소로 자동접속 연결 | `java.open-redirect` | `python.open-redirect` | `ts.open-redirect` | `react.open-redirect-router`<br>`react.target-blank-no-rel`†<br>`react.window-open-no-noopener`†<br>+ ts 규칙 적용 | `vue.open-redirect-router`<br>+ ts 규칙 적용 | `go.open-redirect` |
| IV-08 | 부적절한 XML 외부개체 참조 | `java.xxe` | `python.xxe-lxml` | `ts.xxe` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 해당 없음(표준 encoding/xml은 외부 엔티티를 해석하지 않음) |
| IV-09 | XML 삽입 | 수동 확인(규칙 미작성) | 부분: `python.xml-string-build` | 수동 확인(규칙 미작성) | 수동 확인(규칙 미작성) | 수동 확인(규칙 미작성) | 부분: `go.xml-string-build` |
| IV-10 | LDAP 삽입 | 부분: `java.ldap-injection` | 부분: `python.ldap-injection` | 부분: `ts.ldap-injection` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 부분: `go.ldap-injection` |
| IV-11 | 크로스사이트 요청 위조 | 부분: `java.csrf-disabled` | 부분: `python.csrf-disabled` | 부분: `ts.csrf-disabled` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 수동 확인(규칙 미작성) |
| IV-12 | 서버사이드 요청 위조 | `java.ssrf` | `python.ssrf` | `ts.ssrf` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.ssrf` |
| IV-13 | HTTP 응답분할 | 부분: `java.header-injection` | 부분: `python.header-injection` | 부분: `ts.header-injection` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 부분: `go.header-injection` |
| IV-14 | 정수형 오버플로우 | 부분: `java.int-overflow-array-size` | 해당 없음(정수 랩어라운드 없음, BigInt·버퍼 연산은 수동) | 해당 없음(정수 랩어라운드 없음, BigInt·버퍼 연산은 수동) | 해당 없음(정수 랩어라운드 없음, BigInt·버퍼 연산은 수동) | 해당 없음(정수 랩어라운드 없음, BigInt·버퍼 연산은 수동) | 부분: `go.int-overflow` |
| IV-15 | 보안기능 결정에 사용되는 부적절한 입력값 | 부분: `java.unvalidated-security-input` | 부분: `python.client-controlled-auth` | 부분: `ts.client-controlled-role`<br>`ts.message-listener-no-origin`† | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 부분: `go.client-controlled-auth` |
| IV-16 | 메모리 버퍼 오버플로우 | 해당 없음(메모리 관리 언어) | 해당 없음(메모리 관리 언어) | 해당 없음(메모리 관리 언어) | 해당 없음(메모리 관리 언어) | 해당 없음(메모리 관리 언어) | 부분: `go.unsafe-pointer` |
| IV-17 | 포맷 스트링 삽입 | 부분: `java.format-string` | 부분: `python.format-string` | 해당 없음(템플릿 문자열은 서식 지시자를 해석하지 않음) | 해당 없음(템플릿 문자열은 서식 지시자를 해석하지 않음) | 해당 없음(템플릿 문자열은 서식 지시자를 해석하지 않음) | 부분: `go.format-string` |
| SF-01 | 적절한 인증 없는 중요 기능 허용 | 부분: `java.authz-permit-all` | 부분: `python.drf-allow-any` | 부분: `ts.admin-route-no-auth` | ts 규칙이 적용됨 | 부분: `vue.router-guard-always-next`<br>+ ts 규칙 적용 | 부분: `go.admin-route-no-auth` |
| SF-02 | 부적절한 인가 | 수동 확인(대상 객체별 소유권 확인은 코드만으로 판단 불가) | 수동 확인(대상 객체별 소유권 확인은 코드만으로 판단 불가) | 수동 확인(대상 객체별 소유권 확인은 코드만으로 판단 불가) | 수동 확인(대상 객체별 소유권 확인은 코드만으로 판단 불가) | 수동 확인(대상 객체별 소유권 확인은 코드만으로 판단 불가) | 수동 확인(대상 객체별 소유권 확인은 코드만으로 판단 불가) |
| SF-03 | 중요한 자원에 대한 잘못된 권한 설정 | 부분: `java.permissive-file-permission` | 부분: `python.permissive-file-mode` | 부분: `ts.permissive-file-mode` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 부분: `go.permissive-file-mode` |
| SF-04 | 취약한 암호화 알고리즘 사용 | `java.cipher-des`<br>`java.cipher-ecb`<br>`java.weak-hash-md5`<br>`java.weak-hash-sha1` | `python.weak-cipher`<br>`python.weak-hash-md5`<br>`python.weak-hash-sha1` | `ts.weak-cipher`<br>`ts.weak-hash` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.weak-cipher-des`<br>`go.weak-hash-md5`<br>`go.weak-hash-sha1`<br>`go.weak-tls-version` |
| SF-05 | 암호화되지 않은 중요정보 | 부분: `java.plain-http-url` | 부분: `python.cleartext-protocol` | 부분: `ts.localstorage-token`†<br>`ts.plain-http-url`<br>`ts.postmessage-wildcard`† | 부분: `react.insecure-resource-url`<br>`react.token-in-url`†<br>+ ts 규칙 적용 | 부분: `vue.insecure-baseurl`<br>`vue.token-in-route-query`†<br>+ ts 규칙 적용 | 부분: `go.plain-http-url` |
| SF-06 | 하드코드된 중요정보 | `java.hardcoded-crypto-key`<br>`java.hardcoded-password` | `python.hardcoded-jwt-secret`<br>`python.hardcoded-password` | `ts.hardcoded-jose-secret`<br>`ts.hardcoded-jwt-secret`<br>`ts.hardcoded-secret` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.hardcoded-jwt-secret`<br>`go.hardcoded-password` |
| SF-07 | 충분하지 않은 키 길이 사용 | `java.short-key` | `python.short-key` | `ts.short-rsa-key` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.short-key` |
| SF-08 | 적절하지 않은 난수 값 사용 | `java.random-for-security` | `python.insecure-random` | `ts.insecure-random` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.math-rand` |
| SF-09 | 취약한 비밀번호 허용 | 수동 확인(비밀번호 정책은 설정·요구사항 확인) | 수동 확인(비밀번호 정책은 설정·요구사항 확인) | 수동 확인(비밀번호 정책은 설정·요구사항 확인) | 수동 확인(비밀번호 정책은 설정·요구사항 확인) | 수동 확인(비밀번호 정책은 설정·요구사항 확인) | 수동 확인(비밀번호 정책은 설정·요구사항 확인) |
| SF-10 | 부적절한 전자서명 확인 | `java.jwt-no-verify` | `python.jwt-no-verify` | `ts.jwt-decode-no-verify` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.jwt-no-verify` |
| SF-11 | 부적절한 인증서 유효성 검증 | `java.trust-all-certs`<br>`java.trust-all-hostnames` | `python.requests-no-verify`<br>`python.ssl-verify-off` | `ts.tls-verify-off` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.ssh-insecure-hostkey`<br>`go.tls-skip-verify` |
| SF-12 | 쿠키를 통한 정보노출 | `java.cookie-insecure`<br>`java.cookie-no-httponly` | `python.cookie-flags`<br>`python.session-cookie-insecure` | `ts.cookie-insecure`<br>`ts.document-cookie-insecure` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.cookie-flags` |
| SF-13 | 주석문 안에 포함된 시스템 주요정보 | 부분: `java.comment-secret` | 부분: `python.comment-secret` | 부분: `ts.comment-secret` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 부분: `go.comment-secret` |
| SF-14 | 솔트 없이 일방향 해쉬 함수 사용 | `java.unsalted-password-hash` | `python.unsalted-password-hash` | `ts.unsalted-password-hash` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.unsalted-password-hash` |
| SF-15 | 무결성 검사 없는 코드 다운로드 | 부분: `java.remote-code-download` | 수동 확인(규칙 미작성) | 수동 확인(규칙 미작성) | 부분: `react.script-src-no-integrity` | 수동 확인(규칙 미작성) | 부분: `go.plugin-open` |
| SF-16 | 반복된 인증시도 제한 기능 부재 | 수동 확인(인증 시도 제한은 설계·게이트웨이 설정 확인) | 수동 확인(인증 시도 제한은 설계·게이트웨이 설정 확인) | 수동 확인(인증 시도 제한은 설계·게이트웨이 설정 확인) | 수동 확인(인증 시도 제한은 설계·게이트웨이 설정 확인) | 수동 확인(인증 시도 제한은 설계·게이트웨이 설정 확인) | 수동 확인(인증 시도 제한은 설계·게이트웨이 설정 확인) |
| TS-01 | 경쟁조건: 검사시점과 사용시점(TOCTOU) | 부분: `java.toctou-file-exists` | 부분: `python.toctou-exists-then-open` | 부분: `ts.toctou-exists-then-use` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 부분: `go.toctou-stat-then-open` |
| TS-02 | 종료되지 않는 반복문 또는 재귀 함수 | 부분: `java.redos-nested-quantifier` | 부분: `python.redos-nested-quantifier` | 부분: `ts.redos-nested-quantifier` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 해당 없음(regexp는 RE2 선형 시간) / 무한 루프는 수동 |
| EH-01 | 오류 메시지 정보노출 | `java.error-message-response`<br>`java.print-stack-trace` | `python.traceback-response` | `ts.error-leak` | ts 규칙이 적용됨 | `vue.errorhandler-expose`<br>+ ts 규칙 적용 | `go.error-message-response` |
| EH-02 | 오류상황 대응 부재 | `java.empty-catch` | `python.except-pass` | `ts.empty-catch`<br>`ts.fetch-no-catch` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.empty-if-err`<br>`go.ignored-error` |
| EH-03 | 부적절한 예외 처리 | 부분: `java.catch-throwable`<br>`java.throws-generic-exception`† | 부분: `python.bare-except` | 수동 확인(규칙 미작성) | 수동 확인(규칙 미작성) | 수동 확인(규칙 미작성) | 부분: `go.recover-swallow` |
| CE-01 | Null Pointer 역참조 | 부분: `java.null-deref-request-param` | 부분: `python.none-deref-regex` | 해당 없음(TypeScript strictNullChecks 사용 권장) | 해당 없음(TypeScript strictNullChecks 사용 권장) | 해당 없음(TypeScript strictNullChecks 사용 권장) | 부분: `go.unchecked-type-assertion` |
| CE-02 | 부적절한 자원 해제 | 부분: `java.resource-not-closed` | 부분: `python.open-without-with` | 수동 확인(규칙 미작성) | 부분: `react.effect-no-cleanup` | 부분: `vue.lifecycle-no-cleanup` | 부분: `go.resource-not-closed` |
| CE-03 | 해제된 자원 사용 | 해당 없음(해제된 자원 사용은 GC 언어에서 성립하지 않음) | 해당 없음(해제된 자원 사용은 GC 언어에서 성립하지 않음) | 해당 없음(해제된 자원 사용은 GC 언어에서 성립하지 않음) | 해당 없음(해제된 자원 사용은 GC 언어에서 성립하지 않음) | 해당 없음(해제된 자원 사용은 GC 언어에서 성립하지 않음) | 해당 없음(해제된 자원 사용은 GC 언어에서 성립하지 않음) |
| CE-04 | 초기화되지 않은 변수 사용 | 해당 없음(컴파일러·런타임이 점검) | 해당 없음(컴파일러·런타임이 점검) | 해당 없음(컴파일러·런타임이 점검) | 해당 없음(컴파일러·런타임이 점검) | 해당 없음(컴파일러·런타임이 점검) | 해당 없음(컴파일러·런타임이 점검) |
| CE-05 | 신뢰할 수 없는 데이터의 역직렬화 | `java.unsafe-deserialization` | `python.pickle-load`<br>`python.yaml-unsafe-load` | `ts.unsafe-deserialize` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.gob-decode-request` |
| EN-01 | 잘못된 세션에 의한 데이터 정보노출 | 부분: `java.servlet-shared-field` | 부분: `python.flask-global-state` | 수동 확인(규칙 미작성) | 수동 확인(규칙 미작성) | 부분: `vue.data-shared-object` | 수동 확인(규칙 미작성) |
| EN-02 | 제거되지 않고 남은 디버그 코드 | `java.debug-code-leftover` | `python.debug-breakpoint`<br>`python.django-debug`<br>`python.flask-debug` | `ts.debug-leftover`<br>`ts.debugger-statement` | ts 규칙이 적용됨 | `vue.devtools-enabled`<br>+ ts 규칙 적용 | `go.debug-leftover` |
| EN-03 | Public 메소드로부터 반환된 Private 배열 | 부분: `java.private-array-return` | 해당 없음(private 배열 개념 없음) | 해당 없음(private 배열 개념 없음) | 해당 없음(private 배열 개념 없음) | 해당 없음(private 배열 개념 없음) | 부분: `go.exported-slice-return` |
| EN-04 | Private 배열에 Public 데이터 할당 | 부분: `java.private-array-assign` | 해당 없음(private 배열 개념 없음) | 해당 없음(private 배열 개념 없음) | 해당 없음(private 배열 개념 없음) | 해당 없음(private 배열 개념 없음) | 부분: `go.slice-assign-field` |
| AM-01 | DNS lookup에 의존한 보안결정 | 부분: `java.dns-based-decision` | 부분: `python.dns-based-decision` | 부분: `ts.dns-lookup-decision` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 부분: `go.dns-based-decision` |
| AM-02 | 취약한 API 사용 | 부분: `java.deprecated-api` | 부분: `python.deprecated-api` | 부분: `ts.deprecated-api` | 부분: `react.deprecated-react-api`<br>`react.string-ref`<br>+ ts 규칙 적용 | ts 규칙이 적용됨 | 부분: `go.deprecated-api` |
| VC-01 | 클라이언트 번들로 노출되는 비밀키 | 해당 없음(JS/TS 프런트엔드·Node 전용 항목) | 해당 없음(JS/TS 프런트엔드·Node 전용 항목) | `ts.public-env-secret`<br>`ts.service-role-in-client` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 해당 없음(JS/TS 프런트엔드·Node 전용 항목) |
| VC-02 | BaaS 접근통제 미설정(Supabase RLS·Firebase 규칙) | 수동 확인(Supabase RLS·Firebase 규칙은 코드가 아닌 설정으로 판단) | 수동 확인(Supabase RLS·Firebase 규칙은 코드가 아닌 설정으로 판단) | 수동 확인(Supabase RLS·Firebase 규칙은 코드가 아닌 설정으로 판단) | 수동 확인(Supabase RLS·Firebase 규칙은 코드가 아닌 설정으로 판단) | 수동 확인(Supabase RLS·Firebase 규칙은 코드가 아닌 설정으로 판단) | 수동 확인(Supabase RLS·Firebase 규칙은 코드가 아닌 설정으로 판단) |
| VC-03 | 과도하게 열린 CORS | 해당 없음(JS/TS 프런트엔드·Node 전용 항목) | 해당 없음(JS/TS 프런트엔드·Node 전용 항목) | `ts.cors-wildcard` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 해당 없음(JS/TS 프런트엔드·Node 전용 항목) |
| VC-04 | 프로토타입 오염 | 해당 없음(JS/TS 프런트엔드·Node 전용 항목) | 해당 없음(JS/TS 프런트엔드·Node 전용 항목) | `ts.prototype-pollution` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | 해당 없음(JS/TS 프런트엔드·Node 전용 항목) |

## 규칙 수

| 폴더 | 전체 | 행안부 항목에 연결된 보안 규칙 | 품질·버그 규칙 |
|---|---|---|---|
| java | 57 | 51 | 6 |
| python | 57 | 52 | 5 |
| ts | 51 | 46 | 5 |
| react | 21 | 13 | 8 |
| vue | 20 | 11 | 9 |
| go | 57 | 48 | 9 |
| kotlin | 20 | 0 | 20 |

kotlin은 이번 개편 대상이 아니라 기존 규칙을 그대로 둡니다(행안부 항목 연결 없음).
