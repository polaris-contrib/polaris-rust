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
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
    sync::{Arc, RwLock},
};

use crate::core::{
    model::{
        error::{ErrorCode, PoleError},
        naming::Instance,
    },
    plugin::{loadbalance::LoadBalancer, plugins::Plugin},
};

static PLUGIN_NAME: &str = "ringHash";

const DEFAULT_REPLICAS: usize = 5;

/// ConsistentHashLoadBalancer 一致性哈希负载均衡
pub struct ConsistentHashLoadBalancer {
    // 需要把 ring hash 进行一次缓存，避免重复构建 ring hash
    ring_hash_cache: Arc<RwLock<HashMap<String, RingHash>>>,
}

impl ConsistentHashLoadBalancer {
    pub fn builder() -> (fn() -> Box<dyn LoadBalancer>, String) {
        (new_instance, PLUGIN_NAME.to_string())
    }
}

fn new_instance() -> Box<dyn LoadBalancer> {
    Box::new(ConsistentHashLoadBalancer {
        ring_hash_cache: Arc::new(RwLock::new(HashMap::new())),
    })
}

impl Plugin for ConsistentHashLoadBalancer {
    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }

    fn init(&mut self) {}

    fn destroy(&self) {}
}

impl LoadBalancer for ConsistentHashLoadBalancer {
    fn choose_instance(
        &self,
        criteria: crate::core::model::loadbalance::Criteria,
        instances: crate::core::model::naming::ServiceInstances,
    ) -> Result<crate::core::model::naming::Instance, crate::core::model::error::PoleError> {
        if instances.instances.is_empty() {
            return Err(PoleError::new(
                ErrorCode::InstanceNotFound,
                "no instances for ringHash".to_string(),
            ));
        }

        let ring_cache_key = instances.get_cache_key();
        let signature = calc_signature(&instances.instances);

        // 先判断 ring hash 是否已经存在，如果不存在则创建并初始化一个
        {
            let mut ring_hash_cache = self.ring_hash_cache.write().unwrap();
            ring_hash_cache
                .entry(ring_cache_key.clone())
                .and_modify(|ring_hash| {
                    if ring_hash.revision != instances.service.revision
                        || ring_hash.signature != signature
                    {
                        ring_hash.nodes.clear();
                        for instance in instances.instances.iter() {
                            ring_hash.add_node(instance.clone());
                        }
                        ring_hash.revision.clone_from(&instances.service.revision);
                        ring_hash.signature = signature;
                    }
                })
                .or_insert_with(|| {
                    let mut ring_hash = RingHash::new(
                        DEFAULT_REPLICAS,
                        instances.service.revision.clone(),
                        signature,
                    );
                    for instance in instances.instances.iter() {
                        ring_hash.add_node(instance.clone());
                    }
                    ring_hash
                });
        }

        let ring_hash_repo = self.ring_hash_cache.read().unwrap();
        let ring_hash = ring_hash_repo.get(&ring_cache_key).ok_or_else(|| {
            PoleError::new(
                ErrorCode::InternalError,
                "ringHash cache missing after build".to_string(),
            )
        })?;

        let hash = hash(&criteria.hash_key);
        let node = ring_hash.get_node(hash).ok_or_else(|| {
            PoleError::new(
                ErrorCode::InstanceNotFound,
                "ringHash has no nodes".to_string(),
            )
        })?;

        Ok(node.clone())
    }
}

// 定义一个简单的哈希函数
fn hash<T: Hash>(t: &T) -> u64 {
    let mut s = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

// 定义哈希环结构体
struct RingHash {
    nodes: BTreeMap<u64, Instance>,
    replicas: usize,
    revision: String,
    signature: u64,
}

impl RingHash {
    // 创建一个新的哈希环
    fn new(replicas: usize, revision: String, signature: u64) -> Self {
        Self {
            nodes: BTreeMap::new(),
            replicas,
            revision,
            signature,
        }
    }

    // 添加一个节点到哈希环
    fn add_node(&mut self, node: Instance) {
        for i in 0..self.replicas {
            let key = hash(&(node.id.clone(), i));
            self.nodes.insert(key, node.clone());
        }
    }

    // 根据对象的哈希值找到对应的节点
    fn get_node(&self, hash: u64) -> Option<&Instance> {
        self.nodes
            .range(hash..)
            .next()
            .map(|(_, node)| node)
            .or_else(|| self.nodes.iter().next().map(|(_, node)| node))
    }
}

fn calc_signature(instances: &[Instance]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for ins in instances {
        ins.id.hash(&mut hasher);
        ins.weight.hash(&mut hasher);
        ins.health.hash(&mut hasher);
        ins.isolated.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use crate::core::model::{
        loadbalance::Criteria,
        naming::{Instance, ServiceInfo, ServiceInstances},
    };

    use super::ConsistentHashLoadBalancer;

    fn build_instances() -> ServiceInstances {
        let mut v = Vec::new();

        let mut a = Instance::default();
        a.id = "a".to_string();
        a.weight = 100;
        v.push(a);

        let mut b = Instance::default();
        b.id = "b".to_string();
        b.weight = 100;
        v.push(b);

        ServiceInstances::new(
            ServiceInfo {
                revision: "r1".to_string(),
                ..ServiceInfo::default()
            },
            v,
        )
    }

    #[test]
    fn test_ring_hash_should_be_deterministic() {
        let lb = ConsistentHashLoadBalancer::builder().0();
        let criteria = Criteria {
            policy: "ringHash".to_string(),
            hash_key: "user-1".to_string(),
        };

        let first = lb
            .choose_instance(criteria.clone(), build_instances())
            .expect("choose should success");
        let second = lb
            .choose_instance(criteria, build_instances())
            .expect("choose should success");

        assert_eq!(first.id, second.id);
    }

    #[test]
    fn test_ring_hash_empty_instances_should_error() {
        let lb = ConsistentHashLoadBalancer::builder().0();
        let ret = lb.choose_instance(
            Criteria {
                policy: "ringHash".to_string(),
                hash_key: "k".to_string(),
            },
            ServiceInstances::new(ServiceInfo::default(), Vec::new()),
        );
        assert!(ret.is_err());
    }
}
