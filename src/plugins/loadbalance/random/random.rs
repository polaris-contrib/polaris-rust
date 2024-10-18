// Tencent is pleased to support the open source community by making Polaris available.
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

use crate::core::{
    model::{
        error::{ErrorCode, PolarisError},
        naming::Instance,
    },
    plugin::{loadbalance::LoadBalancer, plugins::Plugin},
};

pub struct WeightRandomLoadbalancer {}

impl WeightRandomLoadbalancer {
    pub fn builder() -> (fn() -> Box<dyn LoadBalancer>, String) {
        (new_instance, "weightedRandom".to_string())
    }
}

fn new_instance() -> Box<dyn LoadBalancer> {
    Box::new(WeightRandomLoadbalancer {})
}

impl Plugin for WeightRandomLoadbalancer {
    fn name(&self) -> String {
        "weightedRandom".to_string()
    }

    fn init(&mut self) {}

    fn destroy(&self) {}
}

impl LoadBalancer for WeightRandomLoadbalancer {
    fn choose_instance(
        &self,
        _criteria: crate::core::model::loadbalance::Criteria,
        instances: crate::core::model::naming::ServiceInstances,
    ) -> Result<Instance, PolarisError> {
        let total_weight = instances.total_weight;
        if total_weight == 0 {
            return Err(PolarisError::new(
                ErrorCode::InstanceInfoError,
                "total weight of instances is 0".to_string(),
            ));
        }

        let rand_weight = rand::random::<u64>() % total_weight;
        let mut left: u64 = 0;
        let mut right: u64 = 0;

        for instance in instances.instances.iter() {
            right += instance.weight as u64;
            if rand_weight >= left && rand_weight < right {
                return Ok(instance.clone());
            }
            left = right;
        }

        tracing::debug!(
            "[polaris][loadbalancer][weight_random] choose instance failed, rand_weight: {}",
            rand_weight
        );
        // 随机选取一个
        let index = total_weight % instances.instances.len() as u64;
        Ok(instances.instances[index as usize].clone())
    }
}
