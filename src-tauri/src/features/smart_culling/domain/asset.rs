use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AssetMemberKind {
    Raw,
    Jpeg,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssetCandidate {
    pub path: PathBuf,
    pub kind: AssetMemberKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SkipReason {
    AmbiguousPair,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AssetDecision {
    Eligible {
        primary_path: PathBuf,
        display_path: PathBuf,
        member_paths: Vec<PathBuf>,
    },
    Skipped {
        paths: Vec<PathBuf>,
        reason: SkipReason,
    },
}

pub(crate) fn group_assets(candidates: Vec<AssetCandidate>) -> Vec<AssetDecision> {
    let mut by_stem = BTreeMap::<String, Vec<AssetCandidate>>::new();

    for candidate in candidates {
        let stem = candidate
            .path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_lowercase();
        by_stem.entry(stem).or_default().push(candidate);
    }

    let mut decisions = Vec::new();
    for mut group in by_stem.into_values() {
        group.sort_by(|left, right| left.path.cmp(&right.path));

        let mut raw_members = Vec::new();
        let mut jpeg_members = Vec::new();
        let mut other_members = Vec::new();
        for candidate in group {
            match candidate.kind {
                AssetMemberKind::Raw => raw_members.push(candidate),
                AssetMemberKind::Jpeg => jpeg_members.push(candidate),
                AssetMemberKind::Other => other_members.push(candidate),
            }
        }

        decisions.extend(
            other_members
                .into_iter()
                .map(|candidate| AssetDecision::Eligible {
                    primary_path: candidate.path.clone(),
                    display_path: candidate.path.clone(),
                    member_paths: vec![candidate.path],
                }),
        );

        if raw_members.is_empty() {
            decisions.extend(
                jpeg_members
                    .into_iter()
                    .map(|candidate| AssetDecision::Eligible {
                        primary_path: candidate.path.clone(),
                        display_path: candidate.path.clone(),
                        member_paths: vec![candidate.path],
                    }),
            );
            continue;
        }

        if raw_members.len() == 1 && jpeg_members.len() <= 1 {
            let raw = raw_members.pop().expect("length was checked");
            let primary_path = raw.path.clone();
            let mut member_paths = vec![raw.path];
            let display_path = jpeg_members
                .first()
                .map(|candidate| candidate.path.clone())
                .unwrap_or_else(|| primary_path.clone());
            member_paths.extend(jpeg_members.into_iter().map(|candidate| candidate.path));
            decisions.push(AssetDecision::Eligible {
                primary_path,
                display_path,
                member_paths,
            });
            continue;
        }

        let mut paths = raw_members
            .into_iter()
            .chain(jpeg_members)
            .map(|candidate| candidate.path)
            .collect::<Vec<_>>();
        paths.sort();
        decisions.push(AssetDecision::Skipped {
            paths,
            reason: SkipReason::AmbiguousPair,
        });
    }

    decisions.sort_by(|left, right| decision_path(left).cmp(decision_path(right)));
    decisions
}

fn decision_path(decision: &AssetDecision) -> &PathBuf {
    match decision {
        AssetDecision::Eligible { primary_path, .. } => primary_path,
        AssetDecision::Skipped { paths, .. } => &paths[0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(path: &str, kind: AssetMemberKind) -> AssetCandidate {
        AssetCandidate {
            path: PathBuf::from(path),
            kind,
        }
    }

    #[test]
    fn pairs_one_raw_with_one_jpeg_and_displays_the_jpeg() {
        let decisions = group_assets(vec![
            candidate("/shoot/IMG_0001.jpg", AssetMemberKind::Jpeg),
            candidate("/shoot/IMG_0001.CR3", AssetMemberKind::Raw),
        ]);

        assert_eq!(
            decisions,
            vec![AssetDecision::Eligible {
                primary_path: PathBuf::from("/shoot/IMG_0001.CR3"),
                display_path: PathBuf::from("/shoot/IMG_0001.jpg"),
                member_paths: vec![
                    PathBuf::from("/shoot/IMG_0001.CR3"),
                    PathBuf::from("/shoot/IMG_0001.jpg"),
                ],
            }]
        );
    }

    #[test]
    fn keeps_non_raw_files_as_independent_assets() {
        let decisions = group_assets(vec![
            candidate("/shoot/still.jpg", AssetMemberKind::Jpeg),
            candidate("/shoot/still.png", AssetMemberKind::Other),
        ]);

        assert_eq!(decisions.len(), 2);
        assert!(decisions.iter().all(|decision| matches!(
            decision,
            AssetDecision::Eligible { member_paths, .. } if member_paths.len() == 1
        )));
    }

    #[test]
    fn skips_ambiguous_raw_pair_instead_of_guessing() {
        let decisions = group_assets(vec![
            candidate("/shoot/IMG_0002.CR3", AssetMemberKind::Raw),
            candidate("/shoot/IMG_0002.jpg", AssetMemberKind::Jpeg),
            candidate("/shoot/IMG_0002.jpeg", AssetMemberKind::Jpeg),
        ]);

        assert!(matches!(
            decisions.as_slice(),
            [AssetDecision::Skipped {
                reason: SkipReason::AmbiguousPair,
                ..
            }]
        ));
    }

    #[test]
    fn keeps_raw_and_other_non_raw_with_the_same_stem_independent() {
        let decisions = group_assets(vec![
            candidate("/shoot/IMG_0003.CR3", AssetMemberKind::Raw),
            candidate("/shoot/IMG_0003.png", AssetMemberKind::Other),
        ]);

        assert_eq!(
            decisions,
            vec![
                AssetDecision::Eligible {
                    primary_path: PathBuf::from("/shoot/IMG_0003.CR3"),
                    display_path: PathBuf::from("/shoot/IMG_0003.CR3"),
                    member_paths: vec![PathBuf::from("/shoot/IMG_0003.CR3")],
                },
                AssetDecision::Eligible {
                    primary_path: PathBuf::from("/shoot/IMG_0003.png"),
                    display_path: PathBuf::from("/shoot/IMG_0003.png"),
                    member_paths: vec![PathBuf::from("/shoot/IMG_0003.png")],
                },
            ]
        );
    }

    #[test]
    fn excludes_other_formats_from_raw_jpeg_ambiguity() {
        let decisions = group_assets(vec![
            candidate("/shoot/IMG_0004.CR3", AssetMemberKind::Raw),
            candidate("/shoot/IMG_0004.jpg", AssetMemberKind::Jpeg),
            candidate("/shoot/IMG_0004.jpeg", AssetMemberKind::Jpeg),
            candidate("/shoot/IMG_0004.png", AssetMemberKind::Other),
        ]);

        assert_eq!(decisions.len(), 2);
        assert!(matches!(
            &decisions[0],
            AssetDecision::Skipped {
                paths,
                reason: SkipReason::AmbiguousPair,
            } if paths.len() == 3
        ));
        assert!(matches!(
            &decisions[1],
            AssetDecision::Eligible {
                primary_path,
                display_path,
                member_paths,
            }
                if primary_path == &PathBuf::from("/shoot/IMG_0004.png") && member_paths.len() == 1
                    && display_path == primary_path
        ));
    }
}
