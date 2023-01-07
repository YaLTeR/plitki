//! Object visibility cache with overlap support.

use alloc::{vec, vec::Vec};
use core::ops::Range;

/// Incrementally updated object visibility cache with overlap support.
#[derive(Debug, Clone)]
pub struct VisibilityCache<T: Ord + Copy> {
    // Object start positions, in the original object order.
    start_pos: Vec<T>,
    // Object end positions, in the original object order.
    end_pos: Vec<T>,
    // Contains indices into start_pos.
    sorted_by_start: Vec<usize>,
    // Contains indices into sorted_by_start.
    sorted_by_end: Vec<usize>,
    // Indices into sorted_by_start in the same order as start_pos.
    index_by_start: Vec<usize>,
    // Indices into sorted_by_end in the same order as start_pos.
    index_by_end: Vec<usize>,
    // Whether the object is part of an overlap, in the same order as sorted_by_start and
    // sorted_by_end. If overlap is false, then this object is required to be at the same index by
    // start and by end, thereby its index in sorted_by_start and sorted_by_end is the same. If
    // overlap is true, then the object may be at a different index in sorted_by_start and
    // sorted_by_end; in this case, overlap does not refer to any of the two arrays in particular.
    //
    // It's important to remember that overlap is only about different object order between
    // sorted_by_start and sorted_by_end, and *not* about the object visually overlapping another
    // object. For example, objects (start=0, end=3) and (start=1, end=6) visually overlap, but
    // their order by start and by end is the same, therefore overlap will be false.
    //
    // Consequently, when updating the start and the end positions, as long as the order in
    // sorted_by_start and sorted_by_end does not change, overlap will also not change, even if
    // visual overlap would. For example, if we moved the object above from start=1 to start=4, the
    // visual overlap would disappear, but the values in this array would be unaffected, as the
    // ordering of the objects would not change in either of the arrays.
    overlap: Vec<bool>,
}

impl<T: Ord + Copy> VisibilityCache<T> {
    /// Creates a new visibility cache.
    ///
    /// `objects` is a vector of start and end positions.
    ///
    /// # Panics
    ///
    /// Panics if any object starts after it ends.
    pub fn new(objects: Vec<(T, T)>) -> Self {
        for (start, end) in &objects {
            assert!(start <= end, "an object must not end before it starts");
        }

        let start_pos: Vec<T> = objects.iter().map(|&(start, _)| start).collect();
        let end_pos: Vec<T> = objects.iter().map(|&(_, end)| end).collect();

        let mut sorted_by_start: Vec<usize> = (0..start_pos.len()).collect();
        sorted_by_start.sort_by_key(|&idx| start_pos[idx]);

        let mut index_by_start = vec![0; start_pos.len()];
        for (idx_by_start, &idx) in sorted_by_start.iter().enumerate() {
            index_by_start[idx] = idx_by_start;
        }

        let mut sorted_by_end: Vec<usize> = (0..sorted_by_start.len()).collect();
        sorted_by_end.sort_by_key(|&idx| end_pos[sorted_by_start[idx]]);

        let mut index_by_end = vec![0; start_pos.len()];
        for (idx_by_end, &idx) in sorted_by_end.iter().enumerate() {
            index_by_end[sorted_by_start[idx]] = idx_by_end;
        }

        let overlap = vec![false; start_pos.len()];

        let mut rv = Self {
            start_pos,
            end_pos,
            sorted_by_start,
            sorted_by_end,
            index_by_start,
            index_by_end,
            overlap,
        };
        rv.recompute_overlap(0..rv.sorted_by_end.len());
        rv
    }

    /// Returns the stored start position for an object index.
    #[inline]
    pub fn start_position(&self, object: usize) -> T {
        self.start_pos[object]
    }

    /// Returns the stored end position for an object index.
    #[inline]
    pub fn end_position(&self, object: usize) -> T {
        self.end_pos[object]
    }

    fn recompute_overlap(&mut self, Range { mut start, mut end }: Range<usize>) {
        if start >= end {
            // Early return for empty ranges.
            return;
        }

        // If start or end fall in the middle of an overlapped range, move them to the very start or
        // end of that range respectively. Updating from the middle of an overlap may produce
        // invalid results.
        if self.overlap[start] {
            while start > 0 && self.overlap[start - 1] {
                start -= 1;
            }
        }
        if self.overlap[end - 1] {
            while end < self.overlap.len() && self.overlap[end] {
                end += 1;
            }
        }

        let mut i = start;
        while i < end {
            let mut b_idx = self.sorted_by_end[i];
            if i == b_idx {
                // The arrays are aligned.
                self.overlap[i] = false;
                i += 1;
                continue;
            }

            // Start of a misalignment.
            self.overlap[i] = true;

            // Search for the end.
            i += 1;
            while i < end {
                let new_b_idx = self.sorted_by_end[i];
                b_idx = b_idx.max(new_b_idx);

                // The index matches again, and we've seen all higher indices.
                if i == new_b_idx && i >= b_idx {
                    self.overlap[i] = false;
                    i += 1;
                    break;
                }

                self.overlap[i] = true;
                i += 1;
            }
        }
    }

    /// Verifies internal invariants of the type.
    #[cfg(test)]
    fn verify_invariants(&self) {
        for (start, end) in self.start_pos.iter().zip(&self.end_pos) {
            assert!(start <= end);
        }

        for ab in self.sorted_by_start.windows(2) {
            let (a, b) = (ab[0], ab[1]);
            assert!(self.start_pos[a] <= self.start_pos[b]);
        }

        for ab in self.sorted_by_end.windows(2) {
            let (a, b) = (ab[0], ab[1]);
            assert!(self.end_pos[self.sorted_by_start[a]] <= self.end_pos[self.sorted_by_start[b]]);
        }

        for i in 0..self.start_pos.len() {
            assert_eq!(self.sorted_by_start[self.index_by_start[i]], i);
        }

        for i in 0..self.start_pos.len() {
            assert_eq!(
                self.sorted_by_start[self.sorted_by_end[self.index_by_end[i]]],
                i
            );
        }

        for (i, overlap) in self.overlap.iter().enumerate() {
            if !overlap {
                assert_eq!(i, self.sorted_by_end[i]);
            }
        }
    }

    fn update_start_position(&mut self, object: usize, new_start: T) {
        let old_start = self.start_pos[object];
        if old_start == new_start {
            return;
        }

        // The object's start position has changed. This means its position in the sorted_by_start
        // might need to be updated. However, this change by itself cannot cause a shift in the
        // sorting by the end position, because the start position and the end position are
        // independent. Therefore, the end position update is decoupled into another function.
        //
        // Begin by storing the new start position.
        self.start_pos[object] = new_start;

        if new_start > old_start {
            // Keep track of indices that we changed to recompute overlap.
            let first_changed_idx = self.index_by_start[object];
            // Start with no change.
            let mut one_past_last_changed_idx = first_changed_idx;

            // The object start has moved forward. For example, it is a held LN, the start position
            // of which moves forward every frame.
            //
            // We need to move it forward in the sorted_by_start array correspondingly, if it had
            // moved past any subsequent start position.
            for i in first_changed_idx..self.sorted_by_start.len() - 1 {
                // The object wasn't at the very end, which means there are objects in front
                // of it.
                let next_object = self.sorted_by_start[i + 1];
                if self.start_pos[next_object] >= new_start {
                    // The next object still starts after this object, no need to change anything.
                    break;
                }

                // The next object now starts earlier than this object, swap them.
                self.sorted_by_start[i] = next_object;
                self.sorted_by_start[i + 1] = object;
                // Update corresponding indices in index_by_start.
                self.index_by_start[next_object] = i;
                self.index_by_start[object] = i + 1;
                // Update corresponding indices in sorted_by_end.
                self.sorted_by_end[self.index_by_end[next_object]] = i;
                self.sorted_by_end[self.index_by_end[object]] = i + 1;

                // We swapped the two next values, so put the index two values forward.
                one_past_last_changed_idx = i + 2;
            }

            self.recompute_overlap(first_changed_idx..one_past_last_changed_idx);
        } else {
            // Keep track of indices that we changed to recompute overlap.
            let one_past_last_changed_idx = self.index_by_start[object] + 1;
            // Start with no change.
            let mut first_changed_idx = one_past_last_changed_idx;

            // The object start has moved back. This case is the same as above, but with direction
            // and condition reversed.
            for i in (1..one_past_last_changed_idx).rev() {
                let prev_object = self.sorted_by_start[i - 1];
                if self.start_pos[prev_object] <= new_start {
                    break;
                }

                self.sorted_by_start[i] = prev_object;
                self.sorted_by_start[i - 1] = object;
                self.index_by_start[prev_object] = i;
                self.index_by_start[object] = i - 1;
                self.sorted_by_end[self.index_by_end[prev_object]] = i;
                self.sorted_by_end[self.index_by_end[object]] = i - 1;

                first_changed_idx = i - 1;
            }

            self.recompute_overlap(first_changed_idx..one_past_last_changed_idx);
        }
    }

    fn update_end_position(&mut self, object: usize, new_end: T) {
        let old_end = self.end_pos[object];
        if old_end == new_end {
            return;
        }

        // The object's end position has changed. This means its position in the sorted_by_end might
        // need to be updated. However, this change by itself cannot cause a shift in the sorting by
        // the start position, because the start position and the end position are independent.
        // Therefore, the start position update is decoupled into another function.
        //
        // Begin by storing the new end position.
        self.end_pos[object] = new_end;

        let object_by_start = self.index_by_start[object];
        if new_end > old_end {
            // Keep track of indices that we changed to recompute overlap.
            let first_changed_idx = self.index_by_end[object];
            // Start with no change.
            let mut one_past_last_changed_idx = first_changed_idx;

            // The object end has moved forward. For example, an object's visual style changed,
            // making it taller.
            //
            // We need to move it forward in the sorted_by_end array correspondingly, if it had
            // moved past any subsequent end position.
            for i in first_changed_idx..self.sorted_by_end.len() - 1 {
                // The object wasn't at the very end, which means there are objects in front
                // of it.
                let next_object_by_start = self.sorted_by_end[i + 1];
                let next_object = self.sorted_by_start[next_object_by_start];
                if self.end_pos[next_object] >= new_end {
                    // The next object still ends after this object, no need to change anything.
                    break;
                }

                // The next object now ends earlier than this object, swap them.
                self.sorted_by_end[i] = next_object_by_start;
                self.sorted_by_end[i + 1] = object_by_start;
                // Update corresponding indices in index_by_end.
                self.index_by_end[next_object] = i;
                self.index_by_end[object] = i + 1;

                // We swapped the two next values, so put the index two values forward.
                one_past_last_changed_idx = i + 2;
            }

            self.recompute_overlap(first_changed_idx..one_past_last_changed_idx);
        } else {
            // Keep track of indices that we changed to recompute overlap.
            let one_past_last_changed_idx = self.index_by_end[object] + 1;
            // Start with no change.
            let mut first_changed_idx = one_past_last_changed_idx;

            // The object end has moved back. This case is the same as above, but with direction and
            // condition reversed.
            for i in (1..one_past_last_changed_idx).rev() {
                let prev_object_by_start = self.sorted_by_end[i - 1];
                let prev_object = self.sorted_by_start[prev_object_by_start];
                if self.end_pos[prev_object] <= new_end {
                    break;
                }

                self.sorted_by_end[i] = prev_object_by_start;
                self.sorted_by_end[i - 1] = object_by_start;
                self.index_by_end[prev_object] = i;
                self.index_by_end[object] = i - 1;

                first_changed_idx = i - 1;
            }

            self.recompute_overlap(first_changed_idx..one_past_last_changed_idx);
        }
    }

    /// Updates one of the objects with new start and end positions.
    ///
    /// # Panics
    ///
    /// Panics if `new_start > new_end`.
    #[inline]
    pub fn update(&mut self, object_idx: usize, new_start: T, new_end: T) {
        assert!(
            new_start <= new_end,
            "an object must not end before it starts"
        );
        self.update_start_position(object_idx, new_start);
        self.update_end_position(object_idx, new_end);
    }

    /// Computes and returns objects visible in the given range.
    ///
    /// The returned values are indices into the objects vector passed into
    /// [`VisibilityCache::new`].
    pub fn visible_objects(&self, range: Range<T>) -> impl Iterator<Item = usize> + '_ {
        // Find the first object that ends in or after the visible range. All objects before this
        // one end earlier than the visible range, and therefore are invisible.
        let first_idx_by_end = self
            .sorted_by_end
            .partition_point(|&idx| self.end_pos[self.sorted_by_start[idx]] < range.start);

        // If we're on overlap, walk back until there's no overlap. This is because when overlap is
        // true, the indices between sorted_by_end and sorted_by_start are not interchangeable, so
        // we cannot use first_idx_by_end to index sorted_by_start. However, when overlap is false,
        // the indices *are* interchangeable, so we will be able to use it to index sorted_by_start.
        //
        // You may notice that first_idx_by_start is actually the first index where overlap is true,
        // rather than the last index where overlap is false. This is fine however, because it still
        // gives a conservative estimate of the earliest possible object by start.
        let first_idx_by_start = self.sorted_by_end[..first_idx_by_end]
            .iter()
            .zip(&self.overlap[..first_idx_by_end])
            .enumerate()
            .rev()
            .find(|&(_, (_, overlap))| !overlap)
            .map(|(idx, _)| idx + 1)
            .unwrap_or(0);

        // Find the first object that starts past the visible range. This is the first object that
        // is not yet visible.
        let one_past_last_idx_by_start = self
            .sorted_by_start
            .partition_point(|&idx| self.start_pos[idx] < range.end);

        // one_past_last_idx_by_start does not need overlap adjustment because it is already an
        // index into sorted_by_start!
        self.sorted_by_start[first_idx_by_start..one_past_last_idx_by_start]
            .iter()
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Range;

    use proptest::prelude::*;

    fn check(mut objects: Vec<(u8, u8)>, range: Range<u8>) -> (Vec<bool>, Vec<bool>) {
        for (b, e) in &mut objects {
            *e = (*e).max(*b);
        }

        // println!("objects = {objects:?}, range = {range:?}");

        let naive: Vec<bool> = objects
            .iter()
            .map(|&(b, e)| e >= range.start && b < range.end)
            .collect();

        let optimized = {
            let mut optimized = vec![false; objects.len()];

            let cache = VisibilityCache::new(objects);
            cache.verify_invariants();

            for idx in cache.visible_objects(range) {
                optimized[idx] = true;
            }
            optimized
        };

        (naive, optimized)
    }

    fn check_incremental(
        mut objects: Vec<(u8, u8, Option<i8>, Option<i8>)>,
        range: Range<u8>,
    ) -> (Vec<bool>, Vec<bool>) {
        for (b, e, _, _) in &mut objects {
            *e = (*e).max(*b);
        }

        // println!("objects = {objects:?}, range = {range:?}");

        let mut cache = VisibilityCache::new(
            objects
                .iter()
                .copied()
                .map(|(start, end, _, _)| (start, end))
                .collect(),
        );

        for (idx, (start, end, d_start, d_end)) in objects.iter_mut().enumerate() {
            *start = start.saturating_add_signed(d_start.unwrap_or(0));
            *end = end.saturating_add_signed(d_end.unwrap_or(0)).max(*start);
            if !matches!(d_start, None | Some(0)) || !matches!(d_end, None | Some(0)) {
                cache.update(idx, *start, *end);
                cache.verify_invariants();
            }
        }

        let naive: Vec<bool> = objects
            .iter()
            .map(|&(b, e, _, _)| e >= range.start && b < range.end)
            .collect();

        let optimized = {
            let mut optimized = vec![false; objects.len()];
            for idx in cache.visible_objects(range) {
                optimized[idx] = true;
            }
            optimized
        };

        (naive, optimized)
    }

    #[test]
    fn visibility_cache_is_correct_for_all_small_inputs() {
        // Weeeeeeeeeeeee!
        for b1 in 0..=4 {
            for e1 in b1..=4 {
                for b2 in 0..=4 {
                    for e2 in b2..=4 {
                        for b3 in 0..=4 {
                            for e3 in b3..=4 {
                                for b4 in 0..=4 {
                                    for e4 in b4..=4 {
                                        for start in 0..=2 {
                                            for length in 0..=2 {
                                                let (naive, optimized) = check(
                                                    vec![(b1, e1), (b2, e2), (b3, e3), (b4, e4)],
                                                    start..start + length,
                                                );
                                                // println!(
                                                //     "naive = {naive:?}, optimized = {optimized:?}"
                                                // );
                                                // assert_eq!(naive, optimized);
                                                for (n, o) in naive.into_iter().zip(optimized) {
                                                    assert!(o || !n);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    proptest! {
        #[test]
        fn visibility_cache_is_correct(mut objects: Vec<(u8, u8)>, start in 0u8..245u8) {
            let (naive, optimized) = check(objects, start..start + 10);
            // prop_assert_eq!(naive, optimized);
            for (n, o) in naive.into_iter().zip(optimized) {
                prop_assert!(o || !n);
            }
        }

        #[test]
        fn incremental_visibility_cache_is_correct(
            mut objects: Vec<(u8, u8, Option<i8>, Option<i8>)>,
            start in 0u8..245u8,
        ) {
            let (naive, optimized) = check_incremental(objects, start..start + 10);
            // prop_assert_eq!(naive, optimized);
            for (n, o) in naive.into_iter().zip(optimized) {
                prop_assert!(o || !n);
            }
        }
    }
}
