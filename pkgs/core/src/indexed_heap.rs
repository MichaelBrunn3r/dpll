use std::cmp::Ordering;

/// A binary heap that allows efficient updates of element priorities by their IDs.
pub struct IndexedHeap {
    /// The array representation of the heap.
    heap: Vec<usize>,
    /// Mapping from element ID to its index in the heap.
    id_to_heap_idx: Vec<usize>,
}

impl IndexedHeap {
    const UNSET: usize = usize::MAX;

    /// Creates a new empty IndexedHeap with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        IndexedHeap {
            heap: Vec::with_capacity(capacity),
            id_to_heap_idx: vec![Self::UNSET; capacity],
        }
    }

    /// Checks whether the heap contains an element with the given ID.
    pub fn contains(&self, id: usize) -> bool {
        debug_assert!(id < self.id_to_heap_idx.len());
        unsafe { self.id_to_heap_idx.get_unchecked(id) != &Self::UNSET }
    }

    /// Inserts a new element with the given ID into the heap.
    pub fn insert(&mut self, id: usize, compare: impl Fn(usize, usize) -> Ordering) {
        debug_assert!(id < self.id_to_heap_idx.len());
        debug_assert!(!self.contains(id));

        let idx = self.heap.len();
        self.heap.push(id);
        self.id_to_heap_idx[id] = idx;
        self.sift_up(idx, compare);
    }

    pub fn update(&mut self, id: usize, compare: impl Fn(usize, usize) -> Ordering) {
        debug_assert!(id < self.id_to_heap_idx.len());
        debug_assert!(self.contains(id));

        let idx = self.id_to_heap_idx[id];

        if idx > 0 {
            let parent = self.heap[Self::parent_of(idx)];
            if compare(id, parent) == Ordering::Greater {
                self.sift_up(idx, compare);
                return;
            }
        }

        self.sift_down(idx, compare);
    }

    /// Pops and returns the element with the highest priority from the heap.
    pub fn pop(&mut self, compare: impl Fn(usize, usize) -> Ordering) -> Option<usize> {
        if self.heap.is_empty() {
            return None;
        }

        let first = self.heap[0];
        let last = self.heap.pop().unwrap();

        if !self.heap.is_empty() {
            // Move the last element to the root and sift down
            self.set_heap_at(0, last);
            self.sift_down(0, compare);
        }

        self.id_to_heap_idx[first] = Self::UNSET;
        Some(first)
    }

    /// Sifts the element at `child_idx` up the heap to restore the heap property.
    fn sift_up(&mut self, mut child_idx: usize, compare: impl Fn(usize, usize) -> Ordering) {
        let child = self.heap[child_idx];
        // Sift the child up the heap until the heap property is satisfied
        while child_idx > 0 {
            let parent_idx = Self::parent_of(child_idx);
            let parent = self.heap[parent_idx];

            if compare(child, parent) != Ordering::Greater {
                break; // Child has lower priority than parent => heap property satisfied
            }

            self.set_heap_at(child_idx, parent);
            child_idx = parent_idx;
        }
        self.set_heap_at(child_idx, child);
    }

    /// Sifts the element at `idx` down the heap to restore the heap property.
    fn sift_down(&mut self, mut idx: usize, compare: impl Fn(usize, usize) -> Ordering) {
        let var = self.heap[idx];

        loop {
            let left = Self::left_child_of(idx);
            if left >= self.heap.len() {
                break; // No children
            }
            let right = left + 1;

            // Find the best child
            let mut best_child_idx = left;
            if right < self.heap.len()
                && compare(self.heap[right], self.heap[left]) == Ordering::Greater
            {
                best_child_idx = right;
            }

            let best_child = self.heap[best_child_idx];
            if compare(best_child, var) != Ordering::Greater {
                break; // Parent has higher priority than best child => heap property satisfied
            }

            self.set_heap_at(idx, best_child);
            idx = best_child_idx;
        }
        self.set_heap_at(idx, var);
    }

    /// Sets the heap at index `idx` to `id` and updates the mapping accordingly.
    #[inline(always)]
    fn set_heap_at(&mut self, idx: usize, id: usize) {
        debug_assert!(idx < self.heap.len());
        debug_assert!(id < self.id_to_heap_idx.len());
        unsafe {
            *self.heap.get_unchecked_mut(idx) = id;
            *self.id_to_heap_idx.get_unchecked_mut(id) = idx;
        }
    }

    #[inline(always)]
    fn parent_of(idx: usize) -> usize {
        (idx - 1) >> 1
    }

    #[inline(always)]
    fn left_child_of(idx: usize) -> usize {
        (idx << 1) + 1
    }
}
