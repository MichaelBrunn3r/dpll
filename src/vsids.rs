use crate::{
    lit::{Lit, VariableId},
    partial_assignment::PartialAssignment,
    utils::indexed_heap::IndexedHeap,
};
use std::cmp::Ordering;

/// Variable State Independent Decaying Sum
pub struct VSIDS {
    /// Current activity score for each variable.
    activity_scores: Vec<f64>,
    /// Variables ordered by activity score (highest first).
    heap: IndexedHeap,
    /// The amount by which to bump variable activity scores. Grows over time.
    increment: f64,
    /// The factor by which the increment grows each decay step.
    grow_factor: f64,
}

impl VSIDS {
    pub fn with_scores(initial_var_scores: &[f64]) -> Self {
        let num_vars = initial_var_scores.len();
        let mut heap = IndexedHeap::with_capacity(num_vars);

        let activity_scores = initial_var_scores.to_vec();
        let cmp = |a: usize, b: usize| {
            activity_scores[a]
                .partial_cmp(&activity_scores[b])
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.cmp(&b))
        };
        for (var, _) in activity_scores.iter().enumerate() {
            heap.insert(var, cmp);
        }

        VSIDS {
            activity_scores,
            heap,
            increment: 1.0,
            grow_factor: 1.0 / 0.95,
        }
    }

    /// Selects the unassigned variable with the highest activity score.
    pub fn pop_most_active_unassigned_var(
        &mut self,
        assignment: &PartialAssignment,
    ) -> Option<VariableId> {
        let scores = &self.activity_scores;
        let cmp = |a: usize, b: usize| {
            scores[a]
                .partial_cmp(&scores[b])
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.cmp(&b))
        };

        while let Some(var) = self.heap.pop(cmp) {
            if !assignment.is_assigned(var) {
                return Some(var);
            }
        }
        None
    }

    /// Increases the increment. This makes future bumps more significant than older ones.
    pub fn decay(&mut self) {
        self.increment *= self.grow_factor;
    }

    /// Called when a variable is unassigned (when backtracking), to re-insert it into the heap.
    pub fn on_unassign_var(&mut self, var: VariableId) {
        // Re-insert the variable into the heap with its current activity score.
        if !self.heap.contains(var) {
            let scores = &self.activity_scores;
            let cmp = |a: usize, b: usize| {
                scores[a]
                    .partial_cmp(&scores[b])
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| a.cmp(&b))
            };
            self.heap.insert(var, cmp);
        }
    }

    pub fn bump_lit_activities(&mut self, literals: &[Lit]) {
        self.bump_var_activities(literals.iter().map(|l| l.var()));
    }

    /// Bumps the activity scores of the given variables (usually those involved in a conflict clause).
    pub fn bump_var_activities(&mut self, vars: impl IntoIterator<Item = VariableId>) {
        for var in vars {
            self.activity_scores[var] += self.increment;

            if self.heap.contains(var) {
                let scores = &self.activity_scores;
                self.heap.update(var, |a, b| {
                    scores[a]
                        .partial_cmp(&scores[b])
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.cmp(&b))
                });
            }

            if self.activity_scores[var] > 1e100 {
                self.rescale();
            }
        }
    }

    /// Rescales all activity scores and the increment to prevent numerical overflow.
    pub fn rescale(&mut self) {
        // Scale down all activity scores
        let scale = 1e-100;
        for score in self.activity_scores.iter_mut() {
            *score *= scale;
        }

        // Scale down the increment
        self.increment *= scale;
    }
}
