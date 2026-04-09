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

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Instant,
};

use crate::core::{
    model::error::{ErrorCode, PoleError},
    plugin::{loadbalance::LoadBalancer, plugins::Plugin},
};

static PLUGIN_NAME: &str = "weightedRoundRobin";

/// WeightedRoundRobinBalancer 平滑加权轮询负载均衡
pub struct WeightedRoundRobinBalancer {
    round_robin_cache: Arc<RwLock<HashMap<String, WeightedRoundRobins>>>,
}

impl WeightedRoundRobinBalancer {
    pub fn builder() -> (fn() -> Box<dyn LoadBalancer>, String) {
        (new_instance, PLUGIN_NAME.to_string())
    }
}

fn new_instance() -> Box<dyn LoadBalancer> {
    Box::new(WeightedRoundRobinBalancer {
        round_robin_cache: Arc::new(RwLock::new(HashMap::new())),
    })
}

impl Plugin for WeightedRoundRobinBalancer {
    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }

    fn init(&mut self) {}

    fn destroy(&self) {}
}

impl LoadBalancer for WeightedRoundRobinBalancer {
    fn choose_instance(
        &self,
        _criteria: crate::core::model::loadbalance::Criteria,
        instances: crate::core::model::naming::ServiceInstances,
    ) -> Result<crate::core::model::naming::Instance, crate::core::model::error::PoleError> {
        if instances.instances.is_empty() {
            return Err(PoleError::new(
                ErrorCode::InstanceNotFound,
                "no instances for weightedRoundRobin".to_string(),
            ));
        }

        let cache_key = instances.get_cache_key();
        let mut cache = self.round_robin_cache.write().unwrap();
        let round_robins = cache
            .entry(cache_key)
            .or_insert_with(WeightedRoundRobins::default);

        let mut active_ids = HashMap::<String, ()>::new();
        for ins in &instances.instances {
            active_ids.insert(ins.id.clone(), ());
        }
        round_robins
            .round_robins
            .retain(|id, wrr| active_ids.contains_key(id) && !wrr.is_expire());

        let mut total_weight: i64 = 0;
        let mut selected_id: Option<String> = None;
        let mut selected_weight = i64::MIN;

        for ins in &instances.instances {
            let state = round_robins
                .round_robins
                .entry(ins.id.clone())
                .or_insert_with(|| WeightedRoundRobin::new(ins.weight));

            if state.weight != ins.weight as i64 {
                state.reset(ins.weight);
            }

            if state.weight <= 0 {
                continue;
            }

            state.cur_weight += state.weight;
            state.last_fetch = Instant::now();
            total_weight += state.weight;

            if state.cur_weight > selected_weight {
                selected_weight = state.cur_weight;
                selected_id = Some(ins.id.clone());
            }
        }

        if total_weight <= 0 {
            return Ok(instances.instances[0].clone());
        }

        let selected_id = selected_id.ok_or_else(|| {
            PoleError::new(
                ErrorCode::InternalError,
                "weightedRoundRobin failed to select instance".to_string(),
            )
        })?;

        if let Some(state) = round_robins.round_robins.get_mut(&selected_id) {
            state.cur_weight -= total_weight;
        }

        let selected = instances
            .instances
            .iter()
            .find(|ins| ins.id == selected_id)
            .cloned()
            .ok_or_else(|| {
                PoleError::new(
                    ErrorCode::InstanceNotFound,
                    "selected instance not found".to_string(),
                )
            })?;

        Ok(selected)
    }
}

#[derive(Default)]
struct WeightedRoundRobins {
    round_robins: HashMap<String, WeightedRoundRobin>,
}

struct WeightedRoundRobin {
    cur_weight: i64,
    weight: i64,
    last_fetch: Instant,
}

impl WeightedRoundRobin {
    fn new(weight: u32) -> Self {
        Self {
            cur_weight: 0,
            weight: weight as i64,
            last_fetch: Instant::now(),
        }
    }

    fn reset(&mut self, weight: u32) {
        self.cur_weight = 0;
        self.weight = weight as i64;
    }

    /// is_expire 超过 60s 未被使用则认为过期
    fn is_expire(&self) -> bool {
        self.last_fetch.elapsed().as_secs() > 60
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::core::model::{
        loadbalance::Criteria,
        naming::{Instance, ServiceInfo, ServiceInstances},
    };
    use crate::core::plugin::loadbalance::LoadBalancer;

    use super::WeightedRoundRobinBalancer;

    fn build_instances() -> ServiceInstances {
        let mut v = Vec::new();
        let mut a = Instance::default();
        a.id = "a".to_string();
        a.weight = 5;
        v.push(a);

        let mut b = Instance::default();
        b.id = "b".to_string();
        b.weight = 1;
        v.push(b);

        ServiceInstances::new(ServiceInfo::default(), v)
    }

    #[test]
    fn test_weighted_round_robin_should_prefer_higher_weight() {
        let lb = WeightedRoundRobinBalancer {
            round_robin_cache: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
        };

        let mut hit = HashMap::<String, u32>::new();
        for _ in 0..60 {
            let selected = lb
                .choose_instance(
                    Criteria {
                        policy: "weightedRoundRobin".to_string(),
                        hash_key: String::new(),
                    },
                    build_instances(),
                )
                .expect("choose should success");
            *hit.entry(selected.id).or_insert(0) += 1;
        }

        assert!(hit.get("a").copied().unwrap_or(0) > hit.get("b").copied().unwrap_or(0));
    }
}
