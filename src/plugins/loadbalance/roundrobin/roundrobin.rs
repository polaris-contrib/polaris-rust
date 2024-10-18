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

use std::{
    collections::HashMap,
    sync::{atomic::AtomicU32, Arc, RwLock},
    time::Instant,
};

use crate::core::{
    model::naming::Instance,
    plugin::{loadbalance::LoadBalancer, plugins::Plugin},
};

pub struct WeightedRoundRobinBalancer {
    round_robin_cache: Arc<RwLock<HashMap<String, WeightedRoundRobins>>>,
}

impl WeightedRoundRobinBalancer {
    pub fn builder() -> (fn() -> Box<dyn LoadBalancer>, String) {
        (new_instance, "weightedRoundRobin".to_string())
    }
}

fn new_instance() -> Box<dyn LoadBalancer> {
    Box::new(WeightedRoundRobinBalancer {
        round_robin_cache: Arc::new(RwLock::new(HashMap::new())),
    })
}

impl Plugin for WeightedRoundRobinBalancer {
    fn name(&self) -> String {
        "weightedRoundRobin".to_string()
    }

    fn init(&mut self) {
        todo!()
    }

    fn destroy(&self) {
        todo!()
    }
}

impl LoadBalancer for WeightedRoundRobinBalancer {
    fn choose_instance(
        &self,
        _criteria: crate::core::model::loadbalance::Criteria,
        instances: crate::core::model::naming::ServiceInstances,
    ) -> Result<crate::core::model::naming::Instance, crate::core::model::error::PolarisError> {
        let cache_key = instances.get_cache_key();
        {
            let mut round_robin_cache = self.round_robin_cache.write().unwrap();
            round_robin_cache
                .entry(cache_key.clone())
                .and_modify(|round_robins| {
                    for instance in instances.instances.iter() {
                        let mut inss_round_robin = round_robins.round_robins.write().unwrap();
                        let weight_robin = inss_round_robin
                            .entry(instance.id.clone())
                            .or_insert_with(|| WeightedRoundRobin::new(instance.weight as u32));
                        if weight_robin.is_expire() {
                            round_robins
                                .round_robins
                                .write()
                                .unwrap()
                                .remove(&cache_key);
                            return;
                        }
                        // 如果没过期，但是实例数据出现变化，则更新
                        if weight_robin.get_ins_weight() != instance.weight {
                            weight_robin.reset(instance.weight);
                        }
                    }
                });
        }

        let mut selected_wrr: Option<WeightedRoundRobin> = None;
        let mut selected_ins: Option<&Instance> = None;
        let mut max_weight: i32 = -1;

        let svc_ins_cache_repo = self.round_robin_cache.read().unwrap();
        let svc_ins_cache = svc_ins_cache_repo.get(&cache_key).unwrap();
        for (_, ins) in instances.instances.iter().enumerate() {
            let inss_round_robin = svc_ins_cache.round_robins.write().unwrap();
            let weight_robin = inss_round_robin.get(&ins.id).unwrap();
            let cur_weight = weight_robin.increase_cur_weight();

            weight_robin.update_last_fetch();

            if cur_weight as i32 > max_weight {
                max_weight = cur_weight as i32;
                selected_wrr = Some(weight_robin.clone());
                selected_ins = Some(ins);
            }
        }

        if selected_ins.is_none() {
            // 直接返回第一个
            return Ok(instances.instances[0].clone());
        }

        selected_wrr
            .unwrap()
            .decrease_cur_weight(instances.get_total_weight() as u32);
        Ok(selected_ins.unwrap().clone())
    }
}

struct WeightedRoundRobins {
    round_robins: Arc<RwLock<HashMap<String, WeightedRoundRobin>>>,
}

#[derive(Clone)]
struct WeightedRoundRobin {
    cur_weight: Arc<AtomicU32>,
    weight: Arc<AtomicU32>,
    last_fetch: Arc<RwLock<Instant>>,
}

impl WeightedRoundRobin {
    fn new(weight: u32) -> Self {
        Self {
            cur_weight: Arc::new(AtomicU32::new(0)),
            weight: Arc::new(AtomicU32::new(weight)),
            last_fetch: Arc::new(RwLock::new(Instant::now())),
        }
    }

    fn reset(&self, weight: u32) {
        self.cur_weight
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.weight
            .store(weight, std::sync::atomic::Ordering::Relaxed);
    }

    fn get_ins_weight(&self) -> u32 {
        self.weight.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn increase_cur_weight(&self) -> u32 {
        let cur_weight = self.cur_weight.fetch_add(
            self.weight.load(std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        cur_weight
    }

    fn decrease_cur_weight(&self, weight: u32) {
        self.cur_weight
            .fetch_sub(weight, std::sync::atomic::Ordering::Relaxed);
    }

    fn update_last_fetch(&self) {
        *self.last_fetch.write().unwrap() = Instant::now();
    }

    /// is_expire 超过 60s 未被使用则认为过期
    fn is_expire(&self) -> bool {
        self.last_fetch.read().unwrap().elapsed().as_secs() > 60
    }
}
