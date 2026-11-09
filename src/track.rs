// Ported from SonarSource/sonarqube sonar-core (Tracker.java).
//
// SonarQube
// Copyright (C) SonarSource Sàrl
// mailto:info AT sonarsource DOT com
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 3 of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with this program; if not, write to the Free Software Foundation,
// Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.
//

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub rule: String,
    pub file: String,
    pub line: u32,
    pub line_hash: String,
    pub message: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tracking {
    pub matched: Vec<(usize, usize)>,
    pub new_items: Vec<usize>,
    pub gone: Vec<usize>,
}

/// `Tracker.trackNonClosed`의 매칭 순서. 원본 3번(블록 코드 이동 감지)은 1차에서 제외한다.
fn stage_key(stage: u8, it: &Item) -> String {
    let (r, l, h, m) = (&it.rule, it.line, &it.line_hash, &it.message);
    match stage {
        1 => format!("{r}\0{l}\0{h}\0{m}"),
        2 => format!("{r}\0{l}\0{h}"),
        3 => format!("{r}\0{h}\0{m}"),
        4 => format!("{r}\0{l}\0{m}"),
        _ => format!("{r}\0{h}"),
    }
}

pub fn track(raw: &[Item], base: &[Item]) -> Tracking {
    let mut raw_done = vec![false; raw.len()];
    let mut base_done = vec![false; base.len()];
    let mut matched = Vec::new();
    for stage in 1..=5u8 {
        let mut index: HashMap<String, Vec<usize>> = HashMap::new();
        for (bi, b) in base.iter().enumerate().rev() {
            if !base_done[bi] { index.entry(stage_key(stage, b)).or_default().push(bi); }
        }
        for (ri, r) in raw.iter().enumerate() {
            if raw_done[ri] { continue; }
            if let Some(bi) = index.get_mut(&stage_key(stage, r)).and_then(|v| v.pop()) {
                raw_done[ri] = true;
                base_done[bi] = true;
                matched.push((ri, bi));
            }
        }
    }
    Tracking {
        matched,
        new_items: raw_done.iter().enumerate().filter(|(_, d)| !**d).map(|(i, _)| i).collect(),
        gone: base_done.iter().enumerate().filter(|(_, d)| !**d).map(|(i, _)| i).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i(rule: &str, line: u32, h: &str, m: &str) -> Item {
        Item { rule: rule.into(), file: "f".into(), line, line_hash: h.into(), message: m.into() }
    }

    #[test]
    fn exact_match() {
        let t = track(&[i("r", 1, "h", "m")], &[i("r", 1, "h", "m")]);
        assert_eq!(t, Tracking { matched: vec![(0, 0)], new_items: vec![], gone: vec![] });
    }

    #[test]
    fn moved_line_matches_by_hash() {
        let t = track(&[i("r", 50, "h", "m")], &[i("r", 1, "h", "m")]);
        assert_eq!(t.matched, vec![(0, 0)]);
    }

    #[test]
    fn edited_line_with_same_position_and_message_matches() {
        let t = track(&[i("r", 1, "new", "m")], &[i("r", 1, "old", "m")]);
        assert_eq!(t.matched, vec![(0, 0)]);
    }

    #[test]
    fn changed_message_on_same_line_matches() {
        let t = track(&[i("r", 1, "h", "m2")], &[i("r", 1, "h", "m1")]);
        assert_eq!(t.matched, vec![(0, 0)]);
    }

    #[test]
    fn different_rule_never_matches() {
        let t = track(&[i("a", 1, "h", "m")], &[i("b", 1, "h", "m")]);
        assert!(t.matched.is_empty());
        assert_eq!(t.new_items, vec![0]);
        assert_eq!(t.gone, vec![0]);
    }

    #[test]
    fn tighter_stage_wins_over_looser() {
        // raw[0]은 base[1]과 정확히 같아 먼저 매칭되고, raw[1]이 해시로 base[0]과 매칭된다.
        let raw = [i("r", 5, "h", "m"), i("r", 9, "h", "m")];
        let base = [i("r", 1, "h", "m"), i("r", 5, "h", "m")];
        let mut t = track(&raw, &base);
        t.matched.sort();
        assert_eq!(t.matched, vec![(0, 1), (1, 0)]);
    }

    #[test]
    fn each_base_matches_once() {
        let raw = [i("r", 1, "h", "m"), i("r", 2, "h", "m")];
        let base = [i("r", 1, "h", "m")];
        let t = track(&raw, &base);
        assert_eq!(t.matched.len(), 1);
        assert_eq!(t.new_items, vec![1]);
    }
}
