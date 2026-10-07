use aws_sdk_costexplorer::types::{DateInterval, Expression, Granularity, TagValues};
use aws_sdk_costexplorer::Client as CostClient;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

/// The tag `aws::instance::launch_instance` puts on the instance and its root
/// volume — the key Cost Explorer filters on.
pub const RENTAL_TAG_KEY: &str = "remote-dev-machine-rental-id";

/// `get_rental_actual_cost`'s result. `Unavailable` is an expected outcome
/// (billing lags ~a day; the tag may not be activated), not an error.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ActualCost {
    #[serde(rename_all = "camelCase")]
    Available {
        amount_usd: f64,
        through_date: String,
    },
    #[serde(rename_all = "camelCase")]
    Unavailable { reason: String },
}

/// One day's cost, as returned by Cost Explorer (`start` = YYYY-MM-DD).
pub struct DailyAmount {
    pub start: String,
    pub amount: f64,
}

/// Sums the days that have billing data. Cost Explorer returns a zero-cost
/// row for days it has no data for yet, so `through_date` is the last day
/// with a nonzero amount.
pub fn summarize(days: &[DailyAmount]) -> ActualCost {
    let total: f64 = days.iter().map(|d| d.amount).sum();
    match days.iter().rev().find(|d| d.amount > 0.0) {
        Some(last) => ActualCost::Available {
            amount_usd: total,
            through_date: last.start.clone(),
        },
        None => ActualCost::Unavailable {
            reason: "No billing data yet — AWS reports costs about a day late, and the \
                     remote-dev-machine-rental-id tag must be activated as a cost allocation tag \
                     in the Billing console (it applies from activation onward)."
                .to_string(),
        },
    }
}

fn describe_error(raw: &str) -> String {
    if raw.contains("AccessDenied") || raw.contains("not authorized") {
        "Access denied — the AWS account's credentials need the ce:GetCostAndUsage permission."
            .to_string()
    } else {
        format!("Could not read AWS billing data: {raw}")
    }
}

/// Cost Explorer's date interval is [start, end) in `YYYY-MM-DD`, so the end
/// is tomorrow to include today.
fn interval(launched_at: DateTime<Utc>, now: DateTime<Utc>) -> (String, String) {
    (
        launched_at.format("%Y-%m-%d").to_string(),
        (now + Duration::days(1)).format("%Y-%m-%d").to_string(),
    )
}

/// Billed (not estimated) cost of one rental. Each call is a billed
/// Cost Explorer request ($0.01) — callers must not poll.
pub async fn rental_cost(
    sdk_config: &aws_config::SdkConfig,
    rental_id: &str,
    launched_at: DateTime<Utc>,
) -> ActualCost {
    // Cost Explorer is a global service served from us-east-1.
    let config = sdk_config
        .to_builder()
        .region(aws_config::Region::new("us-east-1"))
        .build();
    let (start, end) = interval(launched_at, Utc::now());

    let period = match DateInterval::builder().start(start).end(end).build() {
        Ok(p) => p,
        Err(e) => {
            return ActualCost::Unavailable {
                reason: e.to_string(),
            }
        }
    };
    let filter = Expression::builder()
        .tags(
            TagValues::builder()
                .key(RENTAL_TAG_KEY)
                .values(rental_id)
                .build(),
        )
        .build();

    let output = CostClient::new(&config)
        .get_cost_and_usage()
        .time_period(period)
        .granularity(Granularity::Daily)
        .metrics("UnblendedCost")
        .filter(filter)
        .send()
        .await;

    match output {
        Ok(out) => {
            let days: Vec<DailyAmount> = out
                .results_by_time()
                .iter()
                .map(|r| DailyAmount {
                    start: r
                        .time_period()
                        .map(|p| p.start().to_string())
                        .unwrap_or_default(),
                    amount: r
                        .total()
                        .and_then(|t| t.get("UnblendedCost"))
                        .and_then(|m| m.amount())
                        .and_then(|a| a.parse().ok())
                        .unwrap_or(0.0),
                })
                .collect();
            summarize(&days)
        }
        Err(e) => ActualCost::Unavailable {
            reason: describe_error(&format!("{e:?}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(start: &str, amount: f64) -> DailyAmount {
        DailyAmount {
            start: start.into(),
            amount,
        }
    }

    #[test]
    fn sums_days_and_reports_the_last_billed_one() {
        let days = [
            day("2026-10-01", 3.5),
            day("2026-10-02", 9.6),
            day("2026-10-03", 0.0),
        ];
        assert_eq!(
            summarize(&days),
            ActualCost::Available {
                amount_usd: 13.1,
                through_date: "2026-10-02".into()
            }
        );
    }

    #[test]
    fn no_billed_days_is_unavailable() {
        assert!(matches!(
            summarize(&[day("2026-10-03", 0.0)]),
            ActualCost::Unavailable { .. }
        ));
        assert!(matches!(summarize(&[]), ActualCost::Unavailable { .. }));
    }

    #[test]
    fn interval_ends_tomorrow() {
        let launched = "2026-10-01T22:30:00Z".parse().unwrap();
        let now = "2026-10-03T01:00:00Z".parse().unwrap();
        assert_eq!(
            interval(launched, now),
            ("2026-10-01".to_string(), "2026-10-04".to_string())
        );
    }

    #[test]
    fn access_denied_gets_an_actionable_message() {
        assert!(describe_error("AccessDeniedException: nope").contains("ce:GetCostAndUsage"));
    }

    #[test]
    fn serializes_with_a_state_tag() {
        let json = serde_json::to_string(&ActualCost::Available {
            amount_usd: 1.0,
            through_date: "2026-10-02".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"state":"available","amountUsd":1.0,"throughDate":"2026-10-02"}"#
        );
        let json = serde_json::to_string(&ActualCost::Unavailable { reason: "x".into() }).unwrap();
        assert_eq!(json, r#"{"state":"unavailable","reason":"x"}"#);
    }
}
