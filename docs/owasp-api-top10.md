# OWASP API Security Top 10 (2023) 정적 분석 대응 범위

qualgate 기본 규칙이 [OWASP API Security Top 10 (2023)](https://owasp.org/API-Security/editions/2023/en/0x11-t10/)의 각 항목을 어디까지 점검하는지 정리한 표입니다. 규칙의 `metadata.qualgate_owasp_api`(예: `["API3:2023"]`)가 이 표의 근거이며, 보고서(html·md·xlsx)의 OWASP 항목별 집계도 같은 값을 씁니다.

## 읽는 법

- qualgate는 **빌드 전 정적 분석**입니다. 파일 하나 안의 코드 패턴과 데이터 흐름만 보며, 파일 간 흐름, 설정 파일(`application.yml`, `.env` 등), 실행 중 동작은 보지 못합니다.
- **판정**
  - **부분 탐지**: 흔한 코드 패턴 일부를 자동으로 찾습니다. 위반 0건이 그 항목의 위험이 없다는 뜻은 아닙니다.
  - **핫스팟**: 확신도가 낮아 사람이 검토해야 합니다. `(H)` 표시 규칙이며 Gitea 이슈에 `qualgate:safe` 라벨로 검토를 끝냅니다.
  - **대상 아님**: 정적 분석으로 판단할 수 없습니다. 설계 검토, 동적 분석(DAST, 예: OWASP ZAP), 운영 점검으로 다룹니다.
- 한 규칙이 여러 항목에 걸리면 각 항목에 모두 셉니다(예: 인증서 검증 끔은 API8·API10).
- `ts` 규칙은 `.js`·`.ts`·`.jsx`·`.tsx`와 Vue `<script>`에 모두 적용됩니다.
- Kotlin은 보안 규칙이 거의 없어 대부분 항목이 비어 있습니다.

## 요약

| 항목 | 이름 | 규칙 수 | 판정 | 정적 분석 밖에서 할 일 |
|---|---|---|---|---|
| API1:2023 | Broken Object Level Authorization | 3 | 핫스팟 | 사용자 A의 토큰으로 B의 객체에 접근하는 다중 계정 테스트 |
| API2:2023 | Broken Authentication | 21 | 부분 탐지 | 로그인 시도 제한, 토큰 폐기·회전, 인증 흐름 점검 |
| API3:2023 | Broken Object Property Level Authorization | 5 | 부분 탐지 | 응답 필드 노출 검토(DTO 설계), 쓰기 가능한 필드 목록 관리 |
| API4:2023 | Unrestricted Resource Consumption | 9 | 부분 탐지 | 요청 빈도 제한(rate limit), 게이트웨이·서버 설정의 크기·시간 제한 |
| API5:2023 | Broken Function Level Authorization | 11 | 부분 탐지·핫스팟 | 역할별 API 접근 테스트, 보안 설정(SecurityFilterChain 등) 검토 |
| API6:2023 | Unrestricted Access to Sensitive Business Flows | 0 | 대상 아님 | 업무 흐름별 남용 시나리오 설계 검토(봇 방지, 구매·예약 제한) |
| API7:2023 | Server Side Request Forgery | 4 | 부분 탐지 | DAST, 서버의 외부 접속 허용 목록(egress) 설정 |
| API8:2023 | Security Misconfiguration | 33 | 부분 탐지 | 설정 파일·보안 헤더·TLS 설정 점검(DAST, 인프라 점검) |
| API9:2023 | Improper Inventory Management | 0 | 대상 아님 | OpenAPI 명세 관리, 구버전·테스트 API 폐기, 게이트웨이 목록 |
| API10:2023 | Unsafe Consumption of APIs | 23 | 부분 탐지 | 외부 API 응답 스키마 검증, 리다이렉트·타임아웃 정책 |

## 항목별 규칙

| 항목 | java | python | ts | react | vue | go | kotlin |
|---|---|---|---|---|---|---|---|
| API1 | `java.bola-find-by-path-id`(H) | `python.bola-get-object`(H) | `ts.bola-find-by-param`(H) | ts 규칙이 적용됨 | ts 규칙이 적용됨 | - | - |
| API2 | `java.jwt-no-verify`<br>`java.jwt-no-expiry`<br>`java.signature-verify-ignored`<br>`java.unsalted-password-hash`<br>`java.ldap-anonymous-bind` | `python.jwt-no-verify`<br>`python.jwt-no-expiry`<br>`python.hardcoded-jwt-secret`<br>`python.unsalted-password-hash` | `ts.jwt-decode-no-verify`<br>`ts.jwt-no-expiry`<br>`ts.hardcoded-jwt-secret`<br>`ts.hardcoded-jose-secret`<br>`ts.localstorage-token`<br>`ts.unsalted-password-hash` | `react.token-in-url`<br>+ ts 규칙 적용 | `vue.token-in-route-query`<br>+ ts 규칙 적용 | `go.jwt-no-verify`<br>`go.jwt-no-expiry`<br>`go.hardcoded-jwt-secret`<br>`go.unsalted-password-hash` | - |
| API3 | `java.mass-assignment-request-body` | `python.mass-assignment-kwargs`<br>`python.drf-serializer-all-fields` | `ts.mass-assignment-body` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.mass-assignment-bind` | - |
| API4 | `java.pageable-unbounded-size`<br>`java.redos-nested-quantifier`<br>`java.int-overflow-array-size` | `python.requests-no-timeout`<br>`python.redos-nested-quantifier` | `ts.multer-no-limits`<br>`ts.redos-nested-quantifier` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.request-body-unbounded`<br>`go.http-no-timeout` | - |
| API5 | `java.authz-permit-all`<br>`java.admin-route-no-auth`(H)<br>`java.unvalidated-security-input` | `python.drf-allow-any`<br>`python.admin-route-no-auth`(H)<br>`python.client-controlled-auth` | `ts.admin-route-no-auth`<br>`ts.client-controlled-role` | ts 규칙이 적용됨 | `vue.router-guard-always-next`<br>+ ts 규칙 적용 | `go.admin-route-no-auth`<br>`go.client-controlled-auth` | - |
| API6 | 대상 아님 | 대상 아님 | 대상 아님 | 대상 아님 | 대상 아님 | 대상 아님 | 대상 아님 |
| API7 | `java.ssrf` | `python.ssrf` | `ts.ssrf` | ts 규칙이 적용됨 | ts 규칙이 적용됨 | `go.ssrf` | - |
| API8 | `java.csrf-disabled`<br>`java.cors-allow-all`(H)<br>`java.error-message-response`<br>`java.print-stack-trace`<br>`java.trust-all-certs`<br>`java.trust-all-hostnames`<br>`java.cookie-no-httponly`<br>`java.cookie-insecure`(H) | `python.csrf-disabled`<br>`python.cors-allow-all`(H)<br>`python.django-debug`<br>`python.flask-debug`<br>`python.traceback-response`<br>`python.requests-no-verify`<br>`python.ssl-verify-off`<br>`python.session-cookie-insecure`<br>`python.cookie-flags`(H) | `ts.csrf-disabled`<br>`ts.cors-wildcard`(H)<br>`ts.error-leak`<br>`ts.tls-verify-off`<br>`ts.cookie-insecure`(H)<br>`ts.document-cookie-insecure`(H) | `react.iframe-no-sandbox`(H)<br>+ ts 규칙 적용 | `vue.devtools-enabled`<br>`vue.errorhandler-expose`<br>+ ts 규칙 적용 | `go.cors-allow-all`(H)<br>`go.error-message-response`<br>`go.tls-skip-verify`<br>`go.ssh-insecure-hostkey`<br>`go.weak-tls-version`<br>`go.cookie-flags`(H) | `kotlin.trust-all-hostnames` |
| API9 | 대상 아님 | 대상 아님 | 대상 아님 | 대상 아님 | 대상 아님 | 대상 아님 | 대상 아님 |
| API10 | `java.trust-all-certs`<br>`java.trust-all-hostnames`<br>`java.plain-http-url`<br>`java.unsafe-deserialization`<br>`java.xxe` | `python.requests-no-timeout`<br>`python.requests-no-verify`<br>`python.ssl-verify-off`<br>`python.cleartext-protocol`<br>`python.pickle-load`<br>`python.yaml-unsafe-load`<br>`python.xxe-lxml` | `ts.tls-verify-off`<br>`ts.plain-http-url`<br>`ts.unsafe-deserialize`<br>`ts.xxe` | `react.insecure-resource-url`<br>+ ts 규칙 적용 | `vue.insecure-baseurl`<br>+ ts 규칙 적용 | `go.tls-skip-verify`<br>`go.ssh-insecure-hostkey`<br>`go.plain-http-url`<br>`go.gob-decode-request` | `kotlin.trust-all-hostnames` |

## 이번에 추가한 규칙(21개)과 탐지 범위

| 규칙 | 항목 | 찾는 것 | 찾지 못하는 것 |
|---|---|---|---|
| `java.mass-assignment-request-body` | API3 | `@RequestBody`로 받은 객체를 그대로 `save`·`persist`·`merge` | 서비스 계층으로 넘긴 뒤 저장(파일 간 흐름) |
| `python.mass-assignment-kwargs` | API3 | `Model(**request.json)` 등 요청 전체를 키워드 인자로 전달, `setattr` 반복 | 중간 변수를 거친 전달 |
| `python.drf-serializer-all-fields` | API3 | DRF `Meta.fields = "__all__"` | `exclude`만 쓴 경우 |
| `ts.mass-assignment-body` | API3 | `create`·`update`·`findByIdAndUpdate`·`Object.assign`·Prisma `data`에 `req.body` 그대로 전달 | 구조 분해 후 전개(`...rest`) |
| `go.mass-assignment-bind` | API3 | `ShouldBindJSON`·`json.Decode`로 받은 구조체를 그대로 `Save`·`Create`·`Updates` | 다른 함수에서 저장 |
| `java.pageable-unbounded-size` | API4 | `@RequestParam` 크기를 `PageRequest.of`·`setMaxResults`에 그대로 사용 | `Pageable` 인자 자동 바인딩(전역 `max-page-size` 설정으로 대응) |
| `ts.multer-no-limits` | API4 | `multer()`에 `limits` 없음 | busboy 등 다른 업로드 라이브러리 |
| `python.requests-no-timeout` | API4·API10 | `requests.get/post/...`에 `timeout` 없음 | `Session` 객체 호출, httpx·aiohttp |
| `go.request-body-unbounded` | API4 | 핸들러에서 `MaxBytesReader` 없이 `io.ReadAll(r.Body)`·`json.NewDecoder(r.Body)` | gin·echo 등 프레임워크 본문 읽기 |
| `java.jwt-no-expiry` | API2 | jjwt `Jwts.builder()`·java-jwt `JWT.create()`에 만료 없음 | 클레임 Map에 exp를 직접 넣은 경우(오탐 가능) |
| `python.jwt-no-expiry` | API2 | `jwt.encode({...})` 딕셔너리에 `"exp"` 없음 | 변수로 넘긴 페이로드 |
| `ts.jwt-no-expiry` | API2 | `jwt.sign`에 `expiresIn`·`exp` 없음 | 변수로 넘긴 옵션 |
| `go.jwt-no-expiry` | API2 | `jwt.MapClaims{...}`에 `"exp"` 없음 | `RegisteredClaims` 구조체 |
| `java.admin-route-no-auth`(H) | API5 | `/admin`·`/internal`·`/manage` 매핑에 `@PreAuthorize`·`@Secured`·`@RolesAllowed` 없음 | SecurityFilterChain 경로 보호(있으면 안전으로 검토 처리) |
| `python.admin-route-no-auth`(H) | API5 | 관리 경로 Flask·FastAPI 라우트에 인증 데코레이터·권한 `Depends` 없음 | 미들웨어·블루프린트 단위 보호 |
| `java.bola-find-by-path-id`(H) | API1 | `@PathVariable` ID로 `findById`·`deleteById` 등을 호출하고 인증 사용자 인자·`@PreAuthorize`가 없음 | 서비스 계층 조회, 조회 후 소유권 비교 |
| `python.bola-get-object`(H) | API1 | 뷰 인자 ID로 `get_object_or_404`·`objects.get`·`query.get`에 소유자 조건 없음 | 조회 후 권한 확인 |
| `ts.bola-find-by-param`(H) | API1 | `req.params` ID로 `findById`·`findByPk`·Prisma `findUnique`/`delete` 등에 사용자 조건 없음 | 조회 후 소유권 비교 |
| `java.cors-allow-all`(H) | API8 | `allowedOrigins("*")`, `addAllowedOrigin("*")`, `Access-Control-Allow-Origin: *` | `@CrossOrigin`(인자 없음), 설정 파일 |
| `python.cors-allow-all`(H) | API8 | `CORS(app)`, `origins="*"`, Django `CORS_ALLOW_ALL_ORIGINS`, FastAPI `allow_origins=["*"]` | `resources` 딕셔너리 안의 `*` |
| `go.cors-allow-all`(H) | API8 | `Access-Control-Allow-Origin: *`, `cors.AllowAll()`·`cors.Default()`, 와일드카드 옵션 | 동적으로 만든 출처 목록 |

새 규칙의 행안부 항목은 가까운 기존 항목(SF-01, SF-02, VC-03)이나 새 확장 항목(VC-05 대량 할당·과도한 속성 노출, VC-06 자원 사용 제한 부재, VC-07 만료 없는 인증 토큰)에 연결했습니다. 자세한 대응표는 [mois49.md](mois49.md)에 있습니다.

## 규칙에 항목을 붙이는 법

규칙 YAML의 `metadata`에 `qualgate_owasp_api`를 목록(또는 쉼표 문자열)으로 적습니다. 값은 `API1:2023`부터 `API10:2023`까지만 허용하며, 다른 값이면 카탈로그를 읽을 때 오류가 납니다.

```yaml
    metadata:
      qualgate_id: java.mass-assignment-request-body
      qualgate_severity: medium
      qualgate_category: security
      qualgate_mois: VC-05
      qualgate_owasp_api: ["API3:2023"]
      cwe: "CWE-915"
```
