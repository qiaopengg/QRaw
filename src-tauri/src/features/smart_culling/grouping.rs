use image_hasher::ImageHash;

const STORY_GAP_MILLIS: i64 = 45_000;
const BURST_GAP_MILLIS: i64 = 2_000;
const VISUAL_GAP_MILLIS: i64 = 12_000;
const ANCHOR_DISTANCE: u32 = 44;
const BURST_DISTANCE: u32 = 64;

pub(crate) struct CaptureDescriptor<'a> {
    pub capture_time_millis: i64,
    pub capture_time_from_exif: bool,
    pub sequence_number: Option<u64>,
    pub hash: &'a ImageHash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CaptureGroup {
    pub story_index: usize,
    pub group_index: usize,
    pub indices: Vec<usize>,
}

pub(crate) fn group_capture_sequence(items: &[CaptureDescriptor<'_>]) -> Vec<CaptureGroup> {
    if items.is_empty() {
        return Vec::new();
    }

    let mut groups = Vec::new();
    let mut story_index = 1;
    let mut group_index = 1;
    let mut current = vec![0];
    let mut anchor = 0;

    for index in 1..items.len() {
        let previous = &items[index - 1];
        let candidate = &items[index];
        let gap = candidate
            .capture_time_millis
            .saturating_sub(previous.capture_time_millis)
            .max(0);
        let anchor_distance = candidate.hash.dist(items[anchor].hash);
        let previous_distance = candidate.hash.dist(previous.hash);
        let sequence_is_close =
            sequence_distance(previous.sequence_number, candidate.sequence_number)
                .is_some_and(|distance| distance <= 2);
        let reliable_capture_time =
            previous.capture_time_from_exif && candidate.capture_time_from_exif;
        let reliable_burst_related = reliable_capture_time
            && gap <= BURST_GAP_MILLIS
            && (sequence_is_close || previous_distance <= BURST_DISTANCE);
        let reliable_visual_related = reliable_capture_time
            && gap <= VISUAL_GAP_MILLIS
            && anchor_distance <= ANCHOR_DISTANCE
            && previous_distance <= ANCHOR_DISTANCE;
        let unverified_visual_sequence = !reliable_capture_time
            && sequence_is_close
            && anchor_distance <= ANCHOR_DISTANCE
            && previous_distance <= ANCHOR_DISTANCE;
        let same_story = if reliable_capture_time {
            gap <= STORY_GAP_MILLIS
        } else {
            unverified_visual_sequence
        };

        if same_story
            && (reliable_burst_related || reliable_visual_related || unverified_visual_sequence)
        {
            current.push(index);
            continue;
        }

        groups.push(CaptureGroup {
            story_index,
            group_index,
            indices: current,
        });
        if !same_story {
            story_index += 1;
            group_index = 1;
        } else {
            group_index += 1;
        }
        current = vec![index];
        anchor = index;
    }

    groups.push(CaptureGroup {
        story_index,
        group_index,
        indices: current,
    });
    groups
}

fn sequence_distance(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    Some(left?.abs_diff(right?))
}

#[cfg(test)]
mod tests {
    use image_hasher::ImageHash;

    use super::*;

    fn hash(bytes: [u8; 8]) -> ImageHash {
        ImageHash::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn sequential_camera_frames_form_one_burst_despite_visual_change() {
        let first = hash([0; 8]);
        let second = hash([255; 8]);
        let items = [
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: true,
                sequence_number: Some(41),
                hash: &first,
            },
            CaptureDescriptor {
                capture_time_millis: 2_000,
                capture_time_from_exif: true,
                sequence_number: Some(42),
                hash: &second,
            },
        ];

        assert_eq!(group_capture_sequence(&items)[0].indices, vec![0, 1]);
    }

    #[test]
    fn a_long_time_gap_starts_a_new_story() {
        let same = hash([0; 8]);
        let items = [
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: true,
                sequence_number: Some(1),
                hash: &same,
            },
            CaptureDescriptor {
                capture_time_millis: 60_000,
                capture_time_from_exif: true,
                sequence_number: Some(2),
                hash: &same,
            },
        ];

        let groups = group_capture_sequence(&items);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].story_index, 1);
        assert_eq!(groups[1].story_index, 2);
    }

    #[test]
    fn random_files_with_matching_times_and_numbers_need_visual_evidence() {
        let first = hash([0; 8]);
        let second = hash([255; 8]);
        let items = [
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: false,
                sequence_number: Some(101),
                hash: &first,
            },
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: false,
                sequence_number: Some(102),
                hash: &second,
            },
        ];

        assert_eq!(group_capture_sequence(&items).len(), 2);
    }

    #[test]
    fn visually_related_sequence_can_group_without_reliable_exif_time() {
        let same = hash([0; 8]);
        let items = [
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: false,
                sequence_number: Some(101),
                hash: &same,
            },
            CaptureDescriptor {
                capture_time_millis: 600_000,
                capture_time_from_exif: false,
                sequence_number: Some(102),
                hash: &same,
            },
        ];

        assert_eq!(group_capture_sequence(&items)[0].indices, vec![0, 1]);
    }

    #[test]
    fn continuous_file_numbers_never_bridge_a_story_gap() {
        let first = hash([0; 8]);
        let second = hash([255; 8]);
        let items = [
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: true,
                sequence_number: Some(101),
                hash: &first,
            },
            CaptureDescriptor {
                capture_time_millis: 60_000,
                capture_time_from_exif: true,
                sequence_number: Some(102),
                hash: &second,
            },
        ];

        assert_eq!(group_capture_sequence(&items).len(), 2);
    }

    #[test]
    fn reliable_sequence_numbers_only_override_visual_change_inside_a_burst() {
        let first = hash([0; 8]);
        let second = hash([255; 8]);
        let items = [
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: true,
                sequence_number: Some(101),
                hash: &first,
            },
            CaptureDescriptor {
                capture_time_millis: 10_000,
                capture_time_from_exif: true,
                sequence_number: Some(102),
                hash: &second,
            },
        ];

        let groups = group_capture_sequence(&items);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].story_index, groups[1].story_index);
    }

    #[test]
    fn anchor_check_prevents_transitive_visual_chains() {
        let anchor = hash([0, 0, 0, 0, 0, 0, 0, 0]);
        let bridge = hash([255, 255, 255, 255, 0, 0, 0, 0]);
        let distant = hash([255; 8]);
        let items = [
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: false,
                sequence_number: Some(1),
                hash: &anchor,
            },
            CaptureDescriptor {
                capture_time_millis: 5_000,
                capture_time_from_exif: false,
                sequence_number: Some(2),
                hash: &bridge,
            },
            CaptureDescriptor {
                capture_time_millis: 9_000,
                capture_time_from_exif: false,
                sequence_number: Some(3),
                hash: &distant,
            },
        ];

        let groups = group_capture_sequence(&items);
        assert_eq!(groups[0].indices, vec![0, 1]);
        assert_eq!(groups[1].indices, vec![2]);
    }

    #[test]
    fn unreliable_equal_timestamps_do_not_merge_unrelated_files() {
        let first = hash([0; 8]);
        let second = hash([255; 8]);
        let items = [
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: false,
                sequence_number: None,
                hash: &first,
            },
            CaptureDescriptor {
                capture_time_millis: 1_000,
                capture_time_from_exif: false,
                sequence_number: None,
                hash: &second,
            },
        ];

        assert_eq!(group_capture_sequence(&items).len(), 2);
    }
}
