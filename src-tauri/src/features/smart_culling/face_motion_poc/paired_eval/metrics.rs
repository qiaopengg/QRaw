#[derive(Default)]
pub(super) struct AgreementMetrics {
    labeled_count: usize,
    exact_count: usize,
    within_one_count: usize,
    usable_match_count: usize,
    absolute_error: usize,
}

impl AgreementMetrics {
    pub(super) fn observe(&mut self, manual_rating: u8, ai_rating: u8) {
        if manual_rating == 0 {
            return;
        }
        let difference = manual_rating.abs_diff(ai_rating) as usize;
        self.labeled_count += 1;
        self.exact_count += usize::from(difference == 0);
        self.within_one_count += usize::from(difference <= 1);
        self.usable_match_count += usize::from((manual_rating >= 3) == (ai_rating >= 3));
        self.absolute_error += difference;
    }

    pub(super) fn print(&self, label: &str) {
        if self.labeled_count == 0 {
            println!("{label} (manual 0 excluded): no decided labeled rows");
            return;
        }
        println!(
            "{label} (manual 0 excluded): n={}, exact={:.1}%, within1={:.1}%, mae={:.3}, usable={:.1}%",
            self.labeled_count,
            percentage(self.exact_count, self.labeled_count),
            percentage(self.within_one_count, self.labeled_count),
            self.absolute_error as f64 / self.labeled_count as f64,
            percentage(self.usable_match_count, self.labeled_count),
        );
    }
}

#[derive(Default)]
pub(super) struct ExpressionComponentMetrics {
    labeled_count: usize,
    scored_count: usize,
    passed_count: usize,
    passed_correct_count: usize,
    failed_count: usize,
    failed_correct_count: usize,
    unknown_count: usize,
    absolute_quality_error: f64,
}

impl ExpressionComponentMetrics {
    pub(super) fn observe(&mut self, manual_rating: u8, expression_score: Option<f64>) {
        if manual_rating == 0 {
            return;
        }
        self.labeled_count += 1;
        match expression_score.filter(|score| score.is_finite() && (0.0..=1.0).contains(score)) {
            Some(score) => {
                self.scored_count += 1;
                let manual_score = f64::from(manual_rating.saturating_sub(1)) / 4.0;
                self.absolute_quality_error += (manual_score - score).abs();
                if score >= 0.5 {
                    self.passed_count += 1;
                    self.passed_correct_count += usize::from(manual_rating >= 3);
                } else {
                    self.failed_count += 1;
                    self.failed_correct_count += usize::from(manual_rating <= 2);
                }
            }
            None => self.unknown_count += 1,
        }
    }

    pub(super) fn print(&self) {
        println!(
            "expression component vs expression-only manual labels (manual 0 excluded): n={}, scored={}/{}, quality_mae={:.3}, pass={}/{} ({:.1}% precision), fail={}/{} ({:.1}% precision), unknown={}/{}",
            self.labeled_count,
            self.scored_count,
            self.labeled_count,
            if self.scored_count == 0 {
                0.0
            } else {
                self.absolute_quality_error / self.scored_count as f64
            },
            self.passed_correct_count,
            self.passed_count,
            percentage_or_zero(self.passed_correct_count, self.passed_count),
            self.failed_correct_count,
            self.failed_count,
            percentage_or_zero(self.failed_correct_count, self.failed_count),
            self.unknown_count,
            self.labeled_count,
        );
    }
}

fn percentage(count: usize, total: usize) -> f64 {
    count as f64 * 100.0 / total as f64
}

fn percentage_or_zero(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        percentage(count, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agreement_excludes_unlabeled_manual_zero() {
        let mut metrics = AgreementMetrics::default();

        metrics.observe(0, 5);
        metrics.observe(3, 3);
        metrics.observe(4, 2);

        assert_eq!(metrics.labeled_count, 2);
        assert_eq!(metrics.exact_count, 1);
        assert_eq!(metrics.within_one_count, 1);
        assert_eq!(metrics.usable_match_count, 1);
        assert_eq!(metrics.absolute_error, 2);
    }

    #[test]
    fn expression_metrics_keep_quality_pass_fail_and_unknown_separate() {
        let mut metrics = ExpressionComponentMetrics::default();

        metrics.observe(0, Some(0.9));
        metrics.observe(3, Some(0.9));
        metrics.observe(1, Some(0.1));
        metrics.observe(4, Some(0.1));
        metrics.observe(2, None);

        assert_eq!(metrics.labeled_count, 4);
        assert_eq!(metrics.scored_count, 3);
        assert_eq!(metrics.passed_count, 1);
        assert_eq!(metrics.passed_correct_count, 1);
        assert_eq!(metrics.failed_count, 2);
        assert_eq!(metrics.failed_correct_count, 1);
        assert_eq!(metrics.unknown_count, 1);
    }
}
