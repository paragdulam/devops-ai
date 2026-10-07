use aws_sdk_pricing::types::{Filter, FilterType};
use aws_sdk_pricing::Client as PricingClient;
use serde_json::Value;

/// Hours in an average month — AWS's own divisor when converting per-GB-month
/// storage prices to hourly.
const HOURS_PER_MONTH: f64 = 730.0;
/// gp3 storage, us-east-1 list price per GB-month.
const GP3_USD_PER_GB_MONTH: f64 = 0.08;
/// Every in-use public IPv4 address, per hour.
const PUBLIC_IPV4_USD_PER_HOUR: f64 = 0.005;

/// Used when the Price List API can't be reached (no `pricing:GetProducts`
/// permission, SCP block, offline). us-east-1 on-demand Linux, USD/hour.
const FALLBACK_INSTANCE_PRICES: &[(&str, f64)] = &[("m5.2xlarge", 0.384)];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RateSource {
    AwsPricing,
    Fallback,
}

impl RateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            RateSource::AwsPricing => "aws-pricing",
            RateSource::Fallback => "fallback",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HourlyRate {
    pub instance_usd: f64,
    pub ebs_usd: f64,
    pub ipv4_usd: f64,
    pub source: RateSource,
}

impl HourlyRate {
    pub fn total(&self) -> f64 {
        self.instance_usd + self.ebs_usd + self.ipv4_usd
    }
}

pub fn ebs_hourly(volume_gb: i32) -> f64 {
    f64::from(volume_gb) * GP3_USD_PER_GB_MONTH / HOURS_PER_MONTH
}

fn fallback_instance_price(instance_type: &str) -> Option<f64> {
    FALLBACK_INSTANCE_PRICES
        .iter()
        .find(|(t, _)| *t == instance_type)
        .map(|(_, p)| *p)
}

/// Pulls the on-demand USD/hour out of one `GetProducts` price-list entry
/// (a JSON document per product).
fn parse_on_demand_hourly(price_list_json: &str) -> Option<f64> {
    let doc: Value = serde_json::from_str(price_list_json).ok()?;
    let on_demand = doc.get("terms")?.get("OnDemand")?.as_object()?;
    for term in on_demand.values() {
        for dim in term.get("priceDimensions")?.as_object()?.values() {
            let usd = dim.get("pricePerUnit")?.get("USD")?.as_str()?;
            let price: f64 = usd.parse().ok()?;
            if price > 0.0 {
                return Some(price);
            }
        }
    }
    None
}

/// The Price List API names regions with display names, not region codes.
fn location_name(region: &str) -> Option<&'static str> {
    Some(match region {
        "us-east-1" => "US East (N. Virginia)",
        "us-east-2" => "US East (Ohio)",
        "us-west-1" => "US West (N. California)",
        "us-west-2" => "US West (Oregon)",
        "ca-central-1" => "Canada (Central)",
        "eu-west-1" => "EU (Ireland)",
        "eu-west-2" => "EU (London)",
        "eu-central-1" => "EU (Frankfurt)",
        "ap-south-1" => "Asia Pacific (Mumbai)",
        "ap-southeast-1" => "Asia Pacific (Singapore)",
        "ap-southeast-2" => "Asia Pacific (Sydney)",
        "ap-northeast-1" => "Asia Pacific (Tokyo)",
        _ => return None,
    })
}

fn term_filter(field: &str, value: &str) -> Filter {
    Filter::builder()
        .r#type(FilterType::TermMatch)
        .field(field)
        .value(value)
        .build()
        .expect("filter has all required fields")
}

async fn fetch_instance_price(
    sdk_config: &aws_config::SdkConfig,
    region: &str,
    instance_type: &str,
) -> Result<f64, String> {
    let location =
        location_name(region).ok_or_else(|| format!("no price-list name for {region}"))?;
    // The Price List API is only served from a couple of regions.
    let config = sdk_config
        .to_builder()
        .region(aws_config::Region::new("us-east-1"))
        .build();
    let output = PricingClient::new(&config)
        .get_products()
        .service_code("AmazonEC2")
        .filters(term_filter("instanceType", instance_type))
        .filters(term_filter("location", location))
        .filters(term_filter("operatingSystem", "Linux"))
        .filters(term_filter("tenancy", "Shared"))
        .filters(term_filter("preInstalledSw", "NA"))
        .filters(term_filter("capacitystatus", "Used"))
        .max_results(5)
        .send()
        .await
        .map_err(|e| format!("Price List API: {e:?}"))?;
    output
        .price_list()
        .iter()
        .find_map(|entry| parse_on_demand_hourly(entry))
        .ok_or_else(|| "Price List API returned no on-demand price".to_string())
}

/// The all-in hourly rate of one rental: instance + root volume + public
/// IPv4. The instance price comes from the Price List API for the account's
/// region, falling back to the built-in us-east-1 table. Errs only when
/// neither source knows the instance type.
pub async fn hourly_rate(
    sdk_config: &aws_config::SdkConfig,
    region: &str,
    instance_type: &str,
    volume_gb: i32,
) -> Result<HourlyRate, String> {
    let (instance_usd, source) = match fetch_instance_price(sdk_config, region, instance_type).await
    {
        Ok(price) => (price, RateSource::AwsPricing),
        Err(e) => {
            eprintln!("pricing: {e}; using built-in table");
            let price = fallback_instance_price(instance_type)
                .ok_or_else(|| format!("No known price for {instance_type}"))?;
            (price, RateSource::Fallback)
        }
    };
    Ok(HourlyRate {
        instance_usd,
        ebs_usd: ebs_hourly(volume_gb),
        ipv4_usd: PUBLIC_IPV4_USD_PER_HOUR,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gp3_monthly_price_becomes_hourly() {
        // 100 GB x $0.08 / 730 h
        assert!((ebs_hourly(100) - 0.010_958).abs() < 1e-5);
    }

    #[test]
    fn total_sums_all_parts() {
        let rate = HourlyRate {
            instance_usd: 0.384,
            ebs_usd: ebs_hourly(100),
            ipv4_usd: PUBLIC_IPV4_USD_PER_HOUR,
            source: RateSource::Fallback,
        };
        assert!((rate.total() - 0.399_958).abs() < 1e-5);
    }

    #[test]
    fn fallback_table_knows_the_standard_profile() {
        assert_eq!(fallback_instance_price("m5.2xlarge"), Some(0.384));
        assert_eq!(fallback_instance_price("x9.mega"), None);
    }

    #[test]
    fn parses_on_demand_price_from_a_price_list_entry() {
        let entry = r#"{
            "product": {"sku": "ABC"},
            "terms": {"OnDemand": {"ABC.JRTCKXETXF": {"priceDimensions": {
                "ABC.JRTCKXETXF.6YS6EN2CT7": {"unit": "Hrs", "pricePerUnit": {"USD": "0.3840000000"}}
            }}}}
        }"#;
        assert_eq!(parse_on_demand_hourly(entry), Some(0.384));
    }

    #[test]
    fn ignores_malformed_or_free_entries() {
        assert_eq!(parse_on_demand_hourly("not json"), None);
        assert_eq!(parse_on_demand_hourly(r#"{"terms":{}}"#), None);
        let free = r#"{"terms":{"OnDemand":{"a":{"priceDimensions":{"b":{"pricePerUnit":{"USD":"0.0"}}}}}}}"#;
        assert_eq!(parse_on_demand_hourly(free), None);
    }

    #[test]
    fn known_regions_have_price_list_names() {
        assert_eq!(location_name("us-east-1"), Some("US East (N. Virginia)"));
        assert_eq!(location_name("mars-north-1"), None);
    }
}
