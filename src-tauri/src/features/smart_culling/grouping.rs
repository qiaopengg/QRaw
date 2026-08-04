use image_hasher::ImageHash;

const EXIF_GAP_MILLIS: i64 = 2_000;
const EXIF_SEQUENCE_DISTANCE: u64 = 2;
const EXIF_VISUAL_DISTANCE: f32 = 0.18;
const FALLBACK_SEQUENCE_DISTANCE: u64 = 1;
const FALLBACK_VISUAL_DISTANCE: f32 = 0.12;
const REVIEW_SEQUENCE_ANCHOR_GAP_MILLIS: i64 = 12_000;
const REVIEW_SEQUENCE_ANCHOR_DISTANCE: u64 = 12;
const REVIEW_SEQUENCE_VISUAL_DISTANCE: f32 = 0.42;

pub(crate) struct CaptureDescriptor<'a> {
    pub capture_time_millis: i64,
    pub capture_time_from_exif: bool,
    pub sequence_number: Option<u64>,
    pub hash: &'a ImageHash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CaptureGroup {
    pub group_index: usize,
    pub indices: Vec<usize>,
    pub requires_manual_review: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Relationship {
    Similar,
    ReviewSequence,
    Independent,
}

pub(crate) fn group_capture_sequence(items: &[CaptureDescriptor<'_>]) -> Vec<CaptureGroup> {
    if items.is_empty() {
        return Vec::new();
    }

    let mut groups = Vec::new();
    let mut current = vec![0];
    let mut anchor = 0;
    let mut requires_manual_review = false;

    for index in 1..items.len() {
        let previous = &items[index - 1];
        let candidate = &items[index];
        match relationship(previous, candidate, &items[anchor]) {
            Relationship::Similar => {
                current.push(index);
                continue;
            }
            Relationship::ReviewSequence => {
                current.push(index);
                requires_manual_review = true;
                continue;
            }
            Relationship::Independent => {}
        }
        groups.push(CaptureGroup {
            group_index: groups.len() + 1,
            indices: current,
            requires_manual_review,
        });
        current = vec![index];
        anchor = index;
        requires_manual_review = false;
    }

    groups.push(CaptureGroup {
        group_index: groups.len() + 1,
        indices: current,
        requires_manual_review,
    });
    groups
}

fn relationship(
    previous: &CaptureDescriptor<'_>,
    candidate: &CaptureDescriptor<'_>,
    anchor: &CaptureDescriptor<'_>,
) -> Relationship {
    let previous_distance = normalized_distance(previous.hash, candidate.hash);
    let anchor_distance = normalized_distance(anchor.hash, candidate.hash);
    let adjacent_sequence_distance =
        sequence_distance(previous.sequence_number, candidate.sequence_number);
    let reliable_exif = previous.capture_time_from_exif
        && candidate.capture_time_from_exif
        && candidate
            .capture_time_millis
            .saturating_sub(previous.capture_time_millis)
            .max(0)
            <= EXIF_GAP_MILLIS;

    if reliable_exif {
        let adjacent_sequence =
            adjacent_sequence_distance.is_some_and(|distance| distance <= EXIF_SEQUENCE_DISTANCE);
        if adjacent_sequence
            && previous_distance <= EXIF_VISUAL_DISTANCE
            && anchor_distance <= EXIF_VISUAL_DISTANCE
        {
            return Relationship::Similar;
        }
        let anchor_gap = candidate
            .capture_time_millis
            .saturating_sub(anchor.capture_time_millis)
            .max(0);
        let anchor_sequence_distance =
            sequence_distance(anchor.sequence_number, candidate.sequence_number);
        if adjacent_sequence
            && anchor.capture_time_from_exif
            && anchor_gap <= REVIEW_SEQUENCE_ANCHOR_GAP_MILLIS
            && anchor_sequence_distance
                .is_some_and(|distance| distance <= REVIEW_SEQUENCE_ANCHOR_DISTANCE)
            && previous_distance <= REVIEW_SEQUENCE_VISUAL_DISTANCE
            && anchor_distance <= REVIEW_SEQUENCE_VISUAL_DISTANCE
        {
            return Relationship::ReviewSequence;
        }
        return Relationship::Independent;
    }

    if !previous.capture_time_from_exif
        && !candidate.capture_time_from_exif
        && adjacent_sequence_distance.is_some_and(|distance| distance <= FALLBACK_SEQUENCE_DISTANCE)
        && previous_distance <= FALLBACK_VISUAL_DISTANCE
        && anchor_distance <= FALLBACK_VISUAL_DISTANCE
    {
        Relationship::Similar
    } else {
        Relationship::Independent
    }
}

fn normalized_distance(left: &ImageHash, right: &ImageHash) -> f32 {
    let bit_count = left.as_bytes().len().min(right.as_bytes().len()) * 8;
    if bit_count == 0 {
        return 1.0;
    }
    left.dist(right) as f32 / bit_count as f32
}

fn sequence_distance(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    Some(left?.abs_diff(right?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(fill: u8) -> ImageHash {
        ImageHash::from_bytes(&[fill; 32]).unwrap()
    }

    fn hash_with_flipped_bits(bit_count: usize) -> ImageHash {
        let mut bytes = [0u8; 32];
        for bit in 0..bit_count.min(256) {
            bytes[bit / 8] |= 1 << (bit % 8);
        }
        ImageHash::from_bytes(&bytes).unwrap()
    }

    fn descriptor<'a>(
        time: i64,
        exif: bool,
        sequence: Option<u64>,
        hash: &'a ImageHash,
    ) -> CaptureDescriptor<'a> {
        CaptureDescriptor {
            capture_time_millis: time,
            capture_time_from_exif: exif,
            sequence_number: sequence,
            hash,
        }
    }

    #[test]
    fn production_hashes_are_256_bits() {
        assert_eq!(hash(0).as_bytes().len() * 8, 256);
    }

    #[test]
    fn reliable_burst_requires_time_sequence_and_visual_evidence() {
        let first = hash(0);
        let close = hash_with_flipped_bits(20);
        let items = [
            descriptor(1_000, true, Some(41), &first),
            descriptor(2_000, true, Some(42), &close),
        ];
        assert_eq!(group_capture_sequence(&items)[0].indices, vec![0, 1]);
    }

    #[test]
    fn visually_different_adjacent_captures_are_manual_review_sequences() {
        let first = hash(0);
        let different = hash_with_flipped_bits(64);
        let items = [
            descriptor(1_000, true, Some(41), &first),
            descriptor(1_100, true, Some(42), &different),
        ];
        let groups = group_capture_sequence(&items);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].requires_manual_review);
    }

    #[test]
    fn visually_unrelated_adjacent_captures_remain_independent() {
        let first = hash(0);
        let different = hash(255);
        let items = [
            descriptor(1_000, true, Some(41), &first),
            descriptor(1_100, true, Some(42), &different),
        ];

        assert_eq!(group_capture_sequence(&items).len(), 2);
    }

    #[test]
    fn similar_photos_across_a_long_gap_do_not_merge() {
        let same = hash(0);
        let items = [
            descriptor(1_000, true, Some(41), &same),
            descriptor(5_000, true, Some(42), &same),
        ];
        assert_eq!(group_capture_sequence(&items).len(), 2);
    }

    #[test]
    fn missing_exif_needs_adjacent_numbers_and_stronger_visual_evidence() {
        let first = hash(0);
        let close = hash_with_flipped_bits(16);
        let items = [
            descriptor(1_000, false, Some(101), &first),
            descriptor(999_000, false, Some(102), &close),
        ];
        assert_eq!(group_capture_sequence(&items)[0].indices, vec![0, 1]);
    }

    #[test]
    fn missing_exif_without_sequence_remains_independent() {
        let same = hash(0);
        let items = [
            descriptor(1_000, false, None, &same),
            descriptor(1_000, false, None, &same),
        ];
        assert_eq!(group_capture_sequence(&items).len(), 2);
    }

    #[test]
    fn anchor_prevents_transitive_visual_drift() {
        let first = hash(0);
        let second = hash_with_flipped_bits(20);
        let third = hash_with_flipped_bits(160);
        let items = [
            descriptor(1_000, true, Some(1), &first),
            descriptor(1_500, true, Some(2), &second),
            descriptor(2_000, true, Some(3), &third),
        ];
        let groups = group_capture_sequence(&items);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].indices, vec![0, 1]);
        assert!(!groups[0].requires_manual_review);
        assert_eq!(groups[1].indices, vec![2]);
    }

    #[test]
    fn manual_review_sequences_still_use_an_anchor_to_prevent_chain_drift() {
        let first = hash(0);
        let different = hash_with_flipped_bits(64);
        let unrelated = hash(255);
        let items = [
            descriptor(1_000, true, Some(1), &first),
            descriptor(2_000, true, Some(2), &different),
            descriptor(3_000, true, Some(3), &unrelated),
        ];

        let groups = group_capture_sequence(&items);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].indices, vec![0, 1]);
        assert!(groups[0].requires_manual_review);
        assert_eq!(groups[1].indices, vec![2]);
    }
}
