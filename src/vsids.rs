use log::error;

use crate::{
    clause::{Lit, VariableId},
    partial_assignment::PartialAssignment,
};
use std::{cmp::Ordering, collections::BinaryHeap};

/// Variable State Independent Decaying Sum
pub struct VSIDS {
    /// Current activity score for each variable.
    activity_scores: Vec<f64>,
    /// Variables ordered by activity score (highest first).
    /// Lazy heap:
    /// - don't actively remove assigned variables in order to avoid costly heap operations
    /// - when assigning a new score to a variable, just push the new score onto the heap => multiple entries for the same variable can exist
    heap: BinaryHeap<Activity>,
    /// The amount by which to bump variable activity scores. Grows over time.
    increment: f64,
    /// The factor by which the increment grows each decay step.
    grow_factor: f64,
}

impl VSIDS {
    pub fn with_scores(initial_var_scores: &[f64]) -> Self {
        let num_vars = initial_var_scores.len();
        let mut heap = BinaryHeap::with_capacity(num_vars);

        let activity_scores = initial_var_scores.to_vec();
        for (var, &score) in activity_scores.iter().enumerate() {
            heap.push(Activity { var, score });
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
        while let Some(Activity { var, score }) = self.heap.pop() {
            // Lazy removal: We don't actively remove assigned variables from the heap.
            // => If we pop an assigned variable, we skip it and continue.
            if assignment.is_assigned(var) {
                continue;
            }

            // Lazy update: If the popped score is outdated, we skip it.
            if score < self.activity_scores[var] {
                continue;
            }

            return Some(var);
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
        self.heap.push(Activity {
            var,
            score: self.activity_scores[var],
        });
    }

    pub fn bump_lit_activities(&mut self, literals: &[Lit]) {
        self.bump_var_activities(literals.iter().map(|l| l.var()));
    }

    /// Bumps the activity scores of the given variables (usually those involved in a conflict clause).
    pub fn bump_var_activities(&mut self, vars: impl IntoIterator<Item = VariableId>) {
        for var in vars {
            self.activity_scores[var] += self.increment;
            self.heap.push(Activity {
                var,
                score: self.activity_scores[var],
            });

            if self.activity_scores[var] > 1e100 {
                self.rescale();
            }
        }
    }

    /// Rescales all activity scores and the increment to prevent numerical overflow.
    pub fn rescale(&mut self) {
        error!("Rescaling VSIDS activity scores to prevent overflow.");

        // Scale down all activity scores
        let scale = 1e-100;
        for score in self.activity_scores.iter_mut() {
            *score *= scale;
        }

        // Scale down the increment
        self.increment *= scale;

        // Rebuild heap to update scores and remove duplicate & stale entries
        self.heap.clear();
        for (var, &score) in self.activity_scores.iter().enumerate() {
            self.heap.push(Activity { var, score });
        }
    }
}

#[derive(PartialOrd)]
struct Activity {
    var: VariableId,
    score: f64,
}

impl Eq for Activity {}
impl PartialEq for Activity {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.var == other.var
    }
}

impl Ord for Activity {
    fn cmp(&self, other: &Self) -> Ordering {
        // 1. Compare scores
        // 2. Compare variable ids
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.var.cmp(&other.var))
    }
}
