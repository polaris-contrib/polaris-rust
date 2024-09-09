use std::{collections::HashMap, sync::Arc, time::Duration};

use polaris_rust::{
    core::model::{error::PolarisError, naming::Location},
    discovery::{
        api::{new_provider_api, ProviderAPI},
        req::{InstanceDeregisterRequest, InstanceHeartbeatRequest, InstanceRegisterRequest},
    },
};
use tracing::level_filters::LevelFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        // all spans/events with a level higher than TRACE (e.g, info, warn, etc.)
        // will be written to stdout.
        .with_thread_names(true)
        .with_file(true)
        .with_level(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_max_level(LevelFilter::DEBUG)
        // sets this to be the default, global collector for this application.
        .init();

    let start_time = std::time::Instant::now();
    let provider_ret = new_provider_api();
    tracing::info!("create provider cost: {:?}", start_time.elapsed());
    match provider_ret {
        Err(err) => {
            tracing::error!("create provider fail: {}", err.to_string());
        }
        Ok(provier) => {
            let metadata = HashMap::new();

            let req = InstanceRegisterRequest {
                flow_id: uuid::Uuid::new_v4().to_string(),
                timeout: Duration::from_secs(1),
                id: None,
                namespace: "rust-demo".to_string(),
                service: "polaris-rust-provider".to_string(),
                ip: "1.1.1.1".to_string(),
                port: 8080,
                vpc_id: "1".to_string(),
                version: "1".to_string(),
                protocol: "1".to_string(),
                health: true,
                isolated: false,
                weight: 100,
                priority: 0,
                metadata: metadata,
                location: Location {
                    region: "1".to_string(),
                    zone: "1".to_string(),
                    campus: "1".to_string(),
                },
                ttl: 5,
                // 这里开启心跳的自动上报能力
                auto_heartbeat: true,
            };
            let _ret = provier.register(req).await;
            match _ret {
                Err(err) => {
                    tracing::error!("register fail: {}", err.to_string());
                }
                Ok(_) => {}
            }

            let arc_provider = Arc::new(provier);

            for _ in 0..120 {
                std::thread::sleep(Duration::from_secs(1));
            }

            // 反注册
            let deregister_req = InstanceDeregisterRequest {
                flow_id: uuid::Uuid::new_v4().to_string(),
                timeout: Duration::from_secs(1),
                namespace: "rust-demo".to_string(),
                service: "polaris-rust-provider".to_string(),
                ip: "1.1.1.1".to_string(),
                port: 8080,
            };

            let _ret = arc_provider.clone().deregister(deregister_req).await;
            match _ret {
                Err(err) => {
                    tracing::error!("deregister fail: {}", err.to_string());
                }
                Ok(_) => {}
            }

            std::mem::forget(arc_provider);
        }
    }

    Ok(())
}
