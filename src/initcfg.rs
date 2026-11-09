//! `init-config`: 설정 저장소의 초기 파일을 만든다.
use crate::catalog::RULE_DIRS;
use crate::settings::DEFAULT_GATE;
use anyhow::{bail, Context};
use std::fmt::Write as _;
use std::path::Path;

pub const DEFAULT_RULESET: &str = "standard";

fn global_toml() -> String {
    let mut s = format!("schema = 1\ndefault_ruleset = \"{DEFAULT_RULESET}\"\n");
    for (m, op, v) in DEFAULT_GATE {
        let op = match op { crate::settings::Op::Gt => "gt", crate::settings::Op::Lt => "lt" };
        write!(s, "\n[[default_gate]]\nmetric = \"{}\"\nop = \"{op}\"\nvalue = {v}\n", m.as_str()).unwrap();
    }
    s
}

fn ruleset_toml() -> String {
    let mut s = format!("schema = 1\nname = \"{DEFAULT_RULESET}\"\ndescription = \"모든 규칙 디렉터리를 켠 기본 룰셋\"\n");
    for d in RULE_DIRS { write!(s, "\n[languages.{d}]\nenabled = true\n").unwrap(); }
    s
}

const README: &str = "# qualgate 설정 저장소

이 저장소의 파일이 qualgate의 룰셋과 프로젝트별 품질 게이트를 정한다. Gitea에서 파일을 직접 편집한다.

```text
qualgate.toml                   전역 설정 (기본 룰셋, 기본 게이트)
rulesets/<name>.toml            룰셋 하나 = 파일 하나
projects/<owner>/<repo>.toml    프로젝트 하나 = 파일 하나
```

## 편집 방법

- 새 룰셋: `rulesets/`에 `<name>.toml`을 만든다. 파일 이름과 `name` 값이 같아야 한다.
- 새 프로젝트: `projects/<owner>/<repo>.toml`을 만들고 `repo`, `ruleset`을 쓴다. 파일이 없는 저장소는 `qualgate.toml`의 기본값으로 스캔한다.
- 규칙 끄기: 룰셋의 `[languages.<dir>]`에서 `enabled = false` 또는 `disabled_rules = [\"java.system-out\"]`.
- 규칙 등급·종류 바꾸기: 룰셋의 `[overrides.\"<규칙 ID>\"]`에 `severity`, `kind`.
- 규칙 ID 목록: `qualgate catalog --json`.

## 검증

커밋하기 전에 검증한다. 오류는 파일과 위치를 함께 출력하고 종료 코드 2로 끝난다.

```sh
qualgate validate-config --config-dir .
```

형식과 검증 규칙의 전체 설명은 qualgate의 `docs/config-format.md`를 본다.
";

const RULESETS_README: &str = "# rulesets

룰셋 하나당 `<name>.toml` 파일 하나. 파일 이름과 파일 안의 `name`이 같아야 한다. `.md` 파일과 점으로 시작하는 파일은 무시된다.
";

const PROJECTS_README: &str = "# projects

프로젝트 하나당 `<owner>/<repo>.toml` 파일 하나. 경로가 Gitea 저장소 전체 이름과 같아야 하고 파일 안의 `repo`와도 같아야 한다. `.md` 파일과 점으로 시작하는 파일은 무시된다.
";

/// `out`에 초기 파일을 쓴다. `out`이 존재하고 비어 있지 않으면 아무것도 쓰지 않고 실패한다.
pub fn init_config(out: &Path) -> anyhow::Result<()> {
    if out.exists() {
        if !out.is_dir() { bail!("{}은(는) 디렉터리가 아닙니다", out.display()); }
        if std::fs::read_dir(out).with_context(|| format!("{} 읽기 실패", out.display()))?.next().is_some() {
            bail!("{}이(가) 비어 있지 않아 쓰지 않습니다", out.display());
        }
    }
    let rs_path = format!("rulesets/{DEFAULT_RULESET}.toml");
    let files = [
        ("qualgate.toml", global_toml()),
        ("README.md", README.to_string()),
        (rs_path.as_str(), ruleset_toml()),
        ("rulesets/README.md", RULESETS_README.to_string()),
        ("projects/README.md", PROJECTS_README.to_string()),
    ];
    for (rel, body) in files {
        let p = out.join(rel);
        std::fs::create_dir_all(p.parent().expect("상위 디렉터리가 있다")).with_context(|| format!("{rel} 디렉터리 생성 실패"))?;
        std::fs::write(&p, body).with_context(|| format!("{} 쓰기 실패", p.display()))?;
    }
    Ok(())
}
