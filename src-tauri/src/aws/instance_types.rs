use aws_sdk_ec2::types::{
    ArchitectureType, Filter, InstanceType, InstanceTypeInfo, SupportedAdditionalProcessorFeature,
};
use aws_sdk_ec2::Client;
use futures::stream::{self, StreamExt};
use serde::Serialize;

use super::instance::ROOT_VOLUME_GB;
use super::pricing;

/// Smallest machine worth offering: the VNC desktop, an IDE and an Android
/// emulator together don't fit in less.
const MIN_VCPUS: i32 = 4;
const MIN_MEMORY_MIB: i64 = 8 * 1024;
/// Concurrent Price List API calls while pricing the list. The API's rate
/// limit is low: 8 at once got most calls throttled.
const PRICING_CONCURRENCY: usize = 2;

/// One selectable instance type, as shown in the machine picker.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceTypeOption {
    pub instance_type: String,
    pub vcpus: i32,
    pub memory_gib: f64,
    /// All-in hourly rate (instance + root volume + public IPv4), or `None`
    /// when neither the Price List API nor the built-in table knows it.
    pub hourly_usd: Option<f64>,
    pub price_source: Option<String>,
}

/// Whether this type can run the Android emulator: x86_64 (the emulator's
/// Linux host builds are x86_64-only), nested virtualization so `/dev/kvm`
/// exists inside the VM, not bare metal (KVM-capable too, but several $/hr),
/// and big enough for desktop + IDE + emulator.
pub fn is_capable(info: &InstanceTypeInfo) -> bool {
    let Some(cpu) = info.processor_info() else {
        return false;
    };
    let x86 = cpu
        .supported_architectures()
        .contains(&ArchitectureType::X8664);
    let nested = cpu
        .supported_features()
        .contains(&SupportedAdditionalProcessorFeature::NestedVirtualization);
    let vcpus = info.v_cpu_info().and_then(|v| v.default_v_cpus()).unwrap_or(0);
    let memory = info.memory_info().and_then(|m| m.size_in_mib()).unwrap_or(0);
    x86 && nested
        && info.bare_metal() != Some(true)
        && vcpus >= MIN_VCPUS
        && memory >= MIN_MEMORY_MIB
}

fn to_option(info: &InstanceTypeInfo) -> Option<InstanceTypeOption> {
    Some(InstanceTypeOption {
        instance_type: info.instance_type()?.as_str().to_string(),
        vcpus: info.v_cpu_info()?.default_v_cpus()?,
        memory_gib: info.memory_info()?.size_in_mib()? as f64 / 1024.0,
        hourly_usd: None,
        price_source: None,
    })
}

/// Cheapest first; unpriced types go last, then by size.
pub fn sort_options(options: &mut [InstanceTypeOption]) {
    options.sort_by(|a, b| {
        let price = |o: &InstanceTypeOption| o.hourly_usd.unwrap_or(f64::INFINITY);
        price(a)
            .total_cmp(&price(b))
            .then(a.vcpus.cmp(&b.vcpus))
            .then(a.memory_gib.total_cmp(&b.memory_gib))
            .then(a.instance_type.cmp(&b.instance_type))
    });
}

/// Every instance type in the client's region that can run the Android
/// emulator, priced and sorted cheapest first.
pub async fn list_emulator_capable(
    ec2: &Client,
    sdk_config: &aws_config::SdkConfig,
    region: &str,
) -> Result<Vec<InstanceTypeOption>, String> {
    let mut pages = ec2
        .describe_instance_types()
        .filters(
            Filter::builder()
                .name("processor-info.supported-architecture")
                .values("x86_64")
                .build(),
        )
        .filters(
            Filter::builder()
                .name("current-generation")
                .values("true")
                .build(),
        )
        .into_paginator()
        .send();

    let mut options = Vec::new();
    while let Some(page) = pages.next().await {
        let page = page.map_err(|e| format!("Could not list instance types: {e:?}"))?;
        options.extend(
            page.instance_types()
                .iter()
                .filter(|i| is_capable(i))
                .filter_map(to_option),
        );
    }

    let mut priced: Vec<InstanceTypeOption> = stream::iter(options)
        .map(|mut option| async move {
            if let Ok(rate) =
                pricing::hourly_rate(sdk_config, region, &option.instance_type, ROOT_VOLUME_GB)
                    .await
            {
                option.hourly_usd = Some(rate.total());
                option.price_source = Some(rate.source.as_str().to_string());
            }
            option
        })
        .buffer_unordered(PRICING_CONCURRENCY)
        .collect()
        .await;

    sort_options(&mut priced);
    Ok(priced)
}

/// Re-checks a single type at launch time, so a stale or hand-edited
/// selection fails up front instead of launching a VM with no `/dev/kvm`.
pub async fn is_emulator_capable(ec2: &Client, instance_type: &str) -> Result<bool, String> {
    let output = ec2
        .describe_instance_types()
        .instance_types(InstanceType::from(instance_type))
        .send()
        .await
        .map_err(|e| format!("Could not look up instance type {instance_type}: {e:?}"))?;
    Ok(output.instance_types().first().is_some_and(is_capable))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_ec2::types::{MemoryInfo, ProcessorInfo, VCpuInfo};

    fn info(
        name: &str,
        arch: ArchitectureType,
        nested: bool,
        bare_metal: bool,
        vcpus: i32,
        mib: i64,
    ) -> InstanceTypeInfo {
        let mut cpu = ProcessorInfo::builder().supported_architectures(arch);
        if nested {
            cpu = cpu.supported_features(SupportedAdditionalProcessorFeature::NestedVirtualization);
        }
        InstanceTypeInfo::builder()
            .instance_type(InstanceType::from(name))
            .processor_info(cpu.build())
            .bare_metal(bare_metal)
            .v_cpu_info(VCpuInfo::builder().default_v_cpus(vcpus).build())
            .memory_info(MemoryInfo::builder().size_in_mib(mib).build())
            .build()
    }

    #[test]
    fn accepts_nested_virt_x86_of_minimum_size() {
        let i = info("m8i-flex.xlarge", ArchitectureType::X8664, true, false, 4, 16384);
        assert!(is_capable(&i));
        let option = to_option(&i).unwrap();
        assert_eq!(option.instance_type, "m8i-flex.xlarge");
        assert_eq!(option.memory_gib, 16.0);
    }

    #[test]
    fn rejects_without_nested_virtualization() {
        assert!(!is_capable(&info("m5.2xlarge", ArchitectureType::X8664, false, false, 8, 32768)));
    }

    #[test]
    fn rejects_bare_metal_arm_and_too_small() {
        assert!(!is_capable(&info("m8i.metal-48xl", ArchitectureType::X8664, true, true, 192, 786432)));
        assert!(!is_capable(&info("m8g.xlarge", ArchitectureType::Arm64, true, false, 4, 16384)));
        assert!(!is_capable(&info("c8i.large", ArchitectureType::X8664, true, false, 2, 4096)));
        assert!(!is_capable(&info("c8i-flex.xlarge", ArchitectureType::X8664, true, false, 4, 4096)));
    }

    #[test]
    fn sorts_cheapest_first_with_unpriced_last() {
        let opt = |t: &str, price: Option<f64>, vcpus: i32| InstanceTypeOption {
            instance_type: t.into(),
            vcpus,
            memory_gib: 16.0,
            hourly_usd: price,
            price_source: None,
        };
        let mut options = vec![
            opt("unpriced", None, 4),
            opt("pricey", Some(0.5), 8),
            opt("cheap", Some(0.2), 4),
        ];
        sort_options(&mut options);
        let names: Vec<_> = options.iter().map(|o| o.instance_type.as_str()).collect();
        assert_eq!(names, ["cheap", "pricey", "unpriced"]);
    }
}
