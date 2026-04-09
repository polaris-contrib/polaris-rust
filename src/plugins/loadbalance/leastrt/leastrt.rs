// Tencent is pleased to support the open source community by making Pole available.
//
// Copyright (C) 2019 THL A29 Limited, a Tencent company. All rights reserved.
//
// Licensed under the BSD 3-Clause License (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://opensource.org/licenses/BSD-3-Clause
//
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use std::sync::Arc;

use crate::core::{
    model::{
        error::{ErrorCode, PoleError},
        instance_stat::{CircuitStatus, InstanceStatManager},
        naming::{Instance, ServiceInstances},
    },
    plugin::{loadbalance::LoadBalancer, plugins::Plugin},
};

static PLUGIN_NAME: &str = "leastRT";

pub struct LeastRtLoadBalancer {
    stat_manager: Option<Arc<InstanceStatManager>>,
}

impl LeastRtLoadBalancer {
    pub fn builder() -> (fn() -> Box<dyn LoadBalancer>, String) {
        (new_instance, PLUGIN_NAME.to_string())
    }
}

fn new_instance() -> Box<dyn LoadBalancer> {
    Box::new(LeastRtLoadBalancer { stat_manager: None })
}

impl Plugin for LeastRtLoadBalancer {
    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }

    fn init(&mut self) {}

    fn destroy(&self) {}
}

impl LoadBalancer for LeastRtLoadBalancer {
    fn set_instance_stat_manager(&mut self, manager: Arc<InstanceStatManager>) {
        self.stat_manager = Some(manager);
    }

    fn choose_instance(
        &self,
        _criteria: crate::core::model::loadbalance::Criteria,
        instances: ServiceInstances,
    ) -> Result<Instance, PoleError> {
        if instances.instances.is_empty() {
            return Err(PoleError::new(
                ErrorCode::InstanceNotFound,
                "no instances for leastRT".to_string(),
            ));
        }

        let manager = match &self.stat_manager {
            Some(v) => v,
            None => return choose_by_weight(&instances),
        };

        let candidates = available_instances(manager, &instances);
        if candidates.is_empty() {
            return Err(PoleError::new(
                ErrorCode::InstanceNotFound,
                "all instances are circuit open for leastRT".to_string(),
            ));
        }

        let mut selected: Option<&Instance> = None;
        let mut selected_rt = u64::MAX;
        let mut has_metric = false;

        for ins in candidates.iter() {
            if let Some(rt) =
                manager.get_ewma_rt_ms(&instances.service.namespace, &instances.service.name, *ins)
            {
                if rt == 0 {
                    continue;
                }
                has_metric = true;
                if rt < selected_rt
                    || (rt == selected_rt
                        && selected
                            .as_ref()
                            .map(|cur| ins.weight > cur.weight)
                            .unwrap_or(true))
                {
                    selected = Some(*ins);
                    selected_rt = rt;
                }
            }
        }

        if has_metric {
            return Ok(selected.unwrap().clone());
        }

        choose_by_weight_instances(candidates)
    }
}

fn choose_by_weight(instances: &ServiceInstances) -> Result<Instance, PoleError> {
    choose_by_weight_instances(instances.instances.iter().collect())
}

fn choose_by_weight_instances(instances: Vec<&Instance>) -> Result<Instance, PoleError> {
    if instances.is_empty() {
        return Err(PoleError::new(
            ErrorCode::InstanceNotFound,
            "no instances for leastRT".to_string(),
        ));
    }

    let total_weight: u64 = instances.iter().map(|ins| ins.weight as u64).sum();
    if total_weight == 0 {
        return Ok(instances[0].clone());
    }

    let rand_weight = rand::random::<u64>() % total_weight;
    let mut range_right = 0_u64;
    for ins in instances.iter() {
        range_right += ins.weight as u64;
        if rand_weight < range_right {
            return Ok((*ins).clone());
        }
    }
    Ok(instances[0].clone())
}

fn available_instances<'a>(
    manager: &InstanceStatManager,
    instances: &'a ServiceInstances,
) -> Vec<&'a Instance> {
    let mut ret = Vec::with_capacity(instances.instances.len());
    for ins in instances.instances.iter() {
        let status =
            manager.get_circuit_status(&instances.service.namespace, &instances.service.name, ins);
        if matches!(status, Some(CircuitStatus::Open)) {
            continue;
        }
        ret.push(ins);
    }
    ret
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::core::model::{
        instance_stat::{CircuitStatus, InstanceStatManager},
        loadbalance::Criteria,
        naming::{Instance, ServiceInfo, ServiceInstances},
    };
    use crate::core::plugin::loadbalance::LoadBalancer;

    use super::LeastRtLoadBalancer;

    #[test]
    fn test_least_rt_selects_min_rt_instance() {
        let mut lb = LeastRtLoadBalancer { stat_manager: None };
        let stat_mgr = Arc::new(InstanceStatManager::new());
        lb.set_instance_stat_manager(stat_mgr.clone());

        let mut a = Instance::default();
        a.id = "a".to_string();

        let mut b = Instance::default();
        b.id = "b".to_string();

        stat_mgr.report_call(
            "default",
            "svc-a",
            &a,
            Duration::from_millis(35),
            true,
            None,
        );
        stat_mgr.report_call("default", "svc-a", &b, Duration::from_millis(8), true, None);

        let ret = lb
            .choose_instance(
                Criteria {
                    policy: "leastRT".to_string(),
                    hash_key: String::new(),
                },
                ServiceInstances::new(
                    ServiceInfo {
                        namespace: "default".to_string(),
                        name: "svc-a".to_string(),
                        ..ServiceInfo::default()
                    },
                    vec![a, b],
                ),
            )
            .expect("choose should success");

        assert_eq!(ret.id, "b");
    }

    #[test]
    fn test_least_rt_should_skip_open_instance() {
        let mut lb = LeastRtLoadBalancer { stat_manager: None };
        let stat_mgr = Arc::new(InstanceStatManager::new());
        lb.set_instance_stat_manager(stat_mgr.clone());

        let mut a = Instance::default();
        a.id = "a".to_string();

        let mut b = Instance::default();
        b.id = "b".to_string();

        stat_mgr.set_circuit_status("default", "svc-a", &a, CircuitStatus::Open);
        stat_mgr.report_call("default", "svc-a", &b, Duration::from_millis(8), true, None);

        let ret = lb
            .choose_instance(
                Criteria {
                    policy: "leastRT".to_string(),
                    hash_key: String::new(),
                },
                ServiceInstances::new(
                    ServiceInfo {
                        namespace: "default".to_string(),
                        name: "svc-a".to_string(),
                        ..ServiceInfo::default()
                    },
                    vec![a, b],
                ),
            )
            .expect("choose should success");

        assert_eq!(ret.id, "b");
    }
}
