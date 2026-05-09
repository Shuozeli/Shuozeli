use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct HealthScore {
    pub total: f64,
    pub recency: f64,
    pub velocity: f64,
    pub issues: f64,
    pub label: HealthLabel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthLabel {
    Active,
    Healthy,
    Moderate,
    Stale,
    Dormant,
}

impl HealthLabel {
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s >= 80.0 => HealthLabel::Active,
            s if s >= 60.0 => HealthLabel::Healthy,
            s if s >= 40.0 => HealthLabel::Moderate,
            s if s >= 20.0 => HealthLabel::Stale,
            _ => HealthLabel::Dormant,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HealthLabel::Active => "Active",
            HealthLabel::Healthy => "Healthy",
            HealthLabel::Moderate => "Moderate",
            HealthLabel::Stale => "Stale",
            HealthLabel::Dormant => "Dormant",
        }
    }

    /// shields.io-compatible color name used to render this label as a badge.
    pub fn shield_color(&self) -> &'static str {
        match self {
            HealthLabel::Active => "brightgreen",
            HealthLabel::Healthy => "green",
            HealthLabel::Moderate => "yellow",
            HealthLabel::Stale => "orange",
            HealthLabel::Dormant => "lightgrey",
        }
    }
}

impl std::fmt::Display for HealthLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn compute_health(
    last_pushed: Option<DateTime<Utc>>,
    commits_last_30d: u32,
    open_issues: u32,
) -> HealthScore {
    let now = Utc::now();

    // Recency (40%): 100 if pushed in last 3 days, decays to 0 at 90 days
    let recency = match last_pushed {
        Some(pushed) => {
            let days = (now - pushed).num_days().max(0) as f64;
            if days <= 3.0 {
                100.0
            } else if days >= 90.0 {
                0.0
            } else {
                // Linear decay from 100 at 3 days to 0 at 90 days
                100.0 * (1.0 - (days - 3.0) / 87.0)
            }
        }
        None => 0.0,
    };

    // Velocity (40%): commits/30 days, 10+ = 100
    let velocity = (commits_last_30d as f64 / 10.0 * 100.0).min(100.0);

    // Issues (20%): 100 if 0 open, -10 per open issue
    let issues = (100.0 - open_issues as f64 * 10.0).max(0.0);

    let total = recency * 0.4 + velocity * 0.4 + issues * 0.2;
    let label = HealthLabel::from_score(total);

    HealthScore {
        total,
        recency,
        velocity,
        issues,
        label,
    }
}
