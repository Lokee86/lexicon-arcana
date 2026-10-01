/// Immutable interval lookup retaining the original first-containing,
/// otherwise nearest-started callable ownership rule.
#[derive(Debug, Clone)]
pub(super) struct CallableRanges {
    sorted: Vec<usize>,
    starts: Vec<u64>,
    maximum_end: Vec<u64>,
    width: usize,
}

impl CallableRanges {
    pub(super) fn new(sorted: Vec<usize>, spans: impl Fn(usize) -> (u64, u64)) -> Self {
        let width = sorted.len().next_power_of_two().max(1);
        let mut maximum_end = vec![0_u64; width * 2];
        let mut starts = Vec::with_capacity(sorted.len());
        for (index, item) in sorted.iter().copied().enumerate() {
            let (start, end) = spans(item);
            starts.push(start);
            maximum_end[width + index] = end;
        }
        for index in (1..width).rev() {
            maximum_end[index] = maximum_end[index * 2].max(maximum_end[index * 2 + 1]);
        }
        Self {
            sorted,
            starts,
            maximum_end,
            width,
        }
    }

    pub(super) fn owner_at(&self, line: u64) -> Option<usize> {
        // The previous scanner considered every callable whose start is at
        // or before this line, returning the *first* containing interval.
        let eligible = self.starts.partition_point(|start| *start <= line);
        if eligible == 0 {
            return None;
        }
        let candidate = self
            .first_containing(1, 0, self.width, eligible, line)
            .unwrap_or(eligible - 1);
        Some(self.sorted[candidate])
    }

    fn first_containing(
        &self,
        position: usize,
        start: usize,
        end: usize,
        eligible: usize,
        line: u64,
    ) -> Option<usize> {
        if start >= eligible || self.maximum_end[position] < line {
            return None;
        }
        if end - start == 1 {
            return Some(start);
        }
        let midpoint = (start + end) / 2;
        self.first_containing(position * 2, start, midpoint, eligible, line)
            .or_else(|| self.first_containing(position * 2 + 1, midpoint, end, eligible, line))
    }
}

#[cfg(test)]
mod tests {
    use super::CallableRanges;

    #[test]
    fn lookup_matches_original_for_nested_overlapping_and_gapped_spans() {
        for ranges in [
            vec![(1, 100), (2, 5), (20, 40), (41, 60)],
            vec![(1, 2), (3, 10), (3, 5), (5, 50), (60, 70)],
            vec![(2, 2), (4, 4), (9, 12)],
            vec![(0, 0), (0, 10), (1, 6), (30, 40)],
            Vec::new(),
        ] {
            let sorted = (0..ranges.len()).collect::<Vec<_>>();
            let index = CallableRanges::new(sorted.clone(), |i| ranges[i]);
            for line in 0..=110 {
                let mut nearest = None;
                let mut expected = None;
                for i in &sorted {
                    let (start, end) = ranges[*i];
                    if start > line {
                        break;
                    }
                    nearest = Some(*i);
                    if end >= line {
                        expected = Some(*i);
                        break;
                    }
                }
                assert_eq!(
                    index.owner_at(line),
                    expected.or(nearest),
                    "line={line}, ranges={ranges:?}"
                );
            }
        }
    }
}
