use std::sync::{Condvar, Mutex};

pub struct ExecutionBudget {
    capacity: usize,
    available: Mutex<usize>,
    condition: Condvar,
}

impl ExecutionBudget {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            available: Mutex::new(capacity),
            condition: Condvar::new(),
        }
    }

    pub fn acquire(&self, weight: usize) -> ExecutionPermit<'_> {
        let weight = self.normalized_weight(weight);
        let mut available = self.available.lock().expect("execution budget poisoned");
        while *available < weight {
            available = self
                .condition
                .wait(available)
                .expect("execution budget poisoned");
        }
        *available -= weight;
        ExecutionPermit {
            budget: self,
            weight,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    fn release(&self, weight: usize) {
        let mut available = self.available.lock().expect("execution budget poisoned");
        *available = (*available + self.normalized_weight(weight)).min(self.capacity);
        drop(available);
        self.condition.notify_all();
    }

    fn normalized_weight(&self, weight: usize) -> usize {
        weight.max(1).min(self.capacity)
    }
}

pub struct ExecutionPermit<'a> {
    budget: &'a ExecutionBudget,
    weight: usize,
}

impl Drop for ExecutionPermit<'_> {
    fn drop(&mut self) {
        self.budget.release(self.weight);
    }
}
