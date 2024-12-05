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
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
    sync::{Arc, RwLock},
};

use crate::core::{
    model::naming::Instance,
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
    ) -> Result<crate::core::model::naming::Instance, crate::core::model::error::PolarisError> {
        let ring_cache_key = instances.get_cache_key();

        // 先判断 ring hash 是否已经存在，如果不存在则创建并初始化一个
        {
            let mut ring_hash_cache = self.ring_hash_cache.write().unwrap();
            ring_hash_cache
                .entry(ring_cache_key.clone())
                .and_modify(|ring_hash| {
                    if ring_hash.revision != instances.service.revision {
                        ring_hash.nodes.clear();
                        for instance in instances.instances.iter() {
                            ring_hash.add_node(instance.clone());
                        }
                        ring_hash.revision.clone_from(&instances.service.revision);
                    }
                })
                .or_insert_with(|| {
                    let mut ring_hash =
                        RingHash::new(DEFAULT_REPLICAS, instances.service.revision.clone());
                    for instance in instances.instances.iter() {
                        ring_hash.add_node(instance.clone());
                    }
                    ring_hash
                });
        }

        let ring_hash_repo = self.ring_hash_cache.read().unwrap();
        let ring_hash = ring_hash_repo.get(&ring_cache_key).unwrap();

        let hash = hash(&criteria.hash_key);
        let node = ring_hash.get_node(hash).unwrap();

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
}

impl RingHash {
    // 创建一个新的哈希环
    fn new(replicas: usize, revision: String) -> Self {
        Self {
            nodes: BTreeMap::new(),
            replicas,
            revision,
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
        self.nodes.range(..=hash).next_back().map(|(_, node)| node)
    }
}
