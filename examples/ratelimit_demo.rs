use std::sync::Arc;
use std::time::Duration;

use tokio;
use tokio::time;

use pole_client_rust::ratelimit::api::{new_ratelimit_api, RateLimitAPI};
use pole_client_rust::ratelimit::req::{QuotaRequest, QuotaResponse};

#[tokio::main]
async fn main() {
    println!("开始测试 Ratelimit 模块...");

    // 创建限流API实例
    match new_ratelimit_api() {
        Ok(api) => {
            println!("✓ 限流API创建成功");

            // 创建配额请求
            let request = QuotaRequest {
                flow_id: "test-flow".to_string(),
                timeout: Duration::from_secs(5),
                service: "test-service".to_string(),
                namespace: "test-namespace".to_string(),
                method: "GET".to_string(),
                traffic_label_provider: |_, _| None,
                external_parameter_supplier: |_| None,
            };

            // 测试获取配额
            match api.get_quota(request.clone()).await {
                Ok(response) => {
                    println!("✓ 配额请求成功: {:?}", response);
                }
                Err(e) => {
                    eprintln!("✗ 配额请求失败: {:?}", e);
                }
            }

            // 进行小规模的并发测试
            println!("开始小规模并发测试...");

            let mut tasks = vec![];
            for i in 0..5 {
                let api = new_ratelimit_api().unwrap(); // 创建新的API实例用于每个任务
                let mut req = request.clone();
                req.flow_id = format!("test-flow-{}", i);

                let task = tokio::spawn(async move {
                    time::sleep(Duration::from_millis(i * 10)).await;
                    match api.get_quota(req).await {
                        Ok(response) => {
                            println!("  请求 {}: {:?}", i, response);
                            response.allowed
                        }
                        Err(e) => {
                            eprintln!("  请求 {} 失败: {:?}", i, e);
                            false
                        }
                    }
                });

                tasks.push(task);
            }

            let results = futures::future::join_all(tasks).await;
            let allowed_count: usize = results
                .into_iter()
                .map(|r| if r.unwrap_or(false) { 1 } else { 0 })
                .sum();

            println!("并发测试完成: 5个请求中有 {} 个被允许", allowed_count);
        }
        Err(e) => {
            eprintln!("✗ 限流API创建失败: {:?}", e);
        }
    }

    println!("Ratelimit 模块测试完成");
}
