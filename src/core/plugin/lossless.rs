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

use crate::core::plugin::plugins::Plugin;
use crate::discovery::req::{BaseInstance, InstanceProperties};

#[async_trait::async_trait]
pub trait LosslessPolicy: Plugin {
    fn build_instance_properties(&self, instance_properties: InstanceProperties);

    async fn lossless_register(
        &self,
        instance: dyn BaseInstance,
        instance_properties: InstanceProperties,
    );

    async fn lossless_deregister(&self, instance: dyn BaseInstance);
}
