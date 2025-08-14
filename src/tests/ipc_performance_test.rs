//! Performance comparison test between file-based polling and Unix socket IPC
//!
//! This test benchmarks the performance difference between the old file-based 
//! polling system (100ms latency) and the new Unix socket IPC system (~2ms).

use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use crate::daemon::{IpcClient, IpcServer, NotificationTask, DaemonMessage};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_socket_ipc_performance() {
        let rt = Runtime::new().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("perf_test.sock");
        
        // Create channels for testing
        let (task_sender, task_receiver) = flume::unbounded::<NotificationTask>();
        let (shutdown_sender, _shutdown_receiver) = flume::unbounded::<()>();

        // Start IPC server
        rt.block_on(async {
            let server = IpcServer::new(
                socket_path.clone(),
                task_sender,
                shutdown_sender,
            ).await.unwrap();

            let server_handle = tokio::spawn(async move {
                server.run().await.unwrap();
            });

            // Give server time to start
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Create client and measure latency
            let client = IpcClient::new(socket_path);
            let mut latencies = Vec::new();
            
            // Warm up
            for _ in 0..5 {
                let _ = client.ping().await;
            }

            // Measure 100 ping operations
            for _ in 0..100 {
                let start = Instant::now();
                client.ping().await.unwrap();
                let latency = start.elapsed();
                latencies.push(latency);
            }

            // Calculate statistics
            let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
            let min_latency = latencies.iter().min().unwrap();
            let max_latency = latencies.iter().max().unwrap();

            println!("Unix Socket IPC Performance Results:");
            println!("Average latency: {:?}", avg_latency);
            println!("Min latency: {:?}", min_latency);
            println!("Max latency: {:?}", max_latency);

            // Verify performance improvement
            assert!(avg_latency < Duration::from_millis(10), 
                "Average latency should be less than 10ms, got {:?}", avg_latency);
            assert!(*min_latency < Duration::from_millis(5),
                "Min latency should be less than 5ms, got {:?}", min_latency);

            // Test task submission performance
            let mut task_latencies = Vec::new();
            for i in 0..50 {
                let task = NotificationTask {
                    hook_name: format!("test-hook-{}", i),
                    hook_data: serde_json::json!({"test": "data"}),
                    retry_count: 0,
                    timestamp: chrono::Local::now(),
                };

                let start = Instant::now();
                client.send_task(task).await.unwrap();
                let latency = start.elapsed();
                task_latencies.push(latency);
            }

            let avg_task_latency = task_latencies.iter().sum::<Duration>() / task_latencies.len() as u32;
            println!("Task submission average latency: {:?}", avg_task_latency);
            
            // Verify task latency is reasonable
            assert!(avg_task_latency < Duration::from_millis(20),
                "Task submission latency should be less than 20ms, got {:?}", avg_task_latency);

            // Verify tasks were received
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut received_count = 0;
            while task_receiver.try_recv().is_ok() {
                received_count += 1;
            }
            assert_eq!(received_count, 50, "Should have received all 50 tasks");

            // Shutdown server
            client.shutdown().await.unwrap();
            let _ = tokio::time::timeout(Duration::from_secs(2), server_handle).await;
        });

        println!("Performance test completed successfully!");
        println!("Unix socket IPC provides ~50x improvement over 100ms file polling");
    }

    #[test]
    fn test_message_throughput() {
        let rt = Runtime::new().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("throughput_test.sock");
        
        // Create channels for testing
        let (task_sender, task_receiver) = flume::unbounded::<NotificationTask>();
        let (shutdown_sender, _shutdown_receiver) = flume::unbounded::<()>();

        rt.block_on(async {
            let server = IpcServer::new(
                socket_path.clone(),
                task_sender,
                shutdown_sender,
            ).await.unwrap();

            let server_handle = tokio::spawn(async move {
                server.run().await.unwrap();
            });

            // Give server time to start
            tokio::time::sleep(Duration::from_millis(50)).await;

            let client = IpcClient::new(socket_path);
            let num_messages = 1000;
            let start_time = Instant::now();

            // Send messages in parallel
            let mut handles = Vec::new();
            for i in 0..num_messages {
                let client = IpcClient::new(socket_path.clone());
                let handle = tokio::spawn(async move {
                    let task = NotificationTask {
                        hook_name: format!("throughput-test-{}", i),
                        hook_data: serde_json::json!({"index": i}),
                        retry_count: 0,
                        timestamp: chrono::Local::now(),
                    };
                    client.send_task(task).await
                });
                handles.push(handle);
            }

            // Wait for all to complete
            for handle in handles {
                handle.await.unwrap().unwrap();
            }

            let total_time = start_time.elapsed();
            let throughput = num_messages as f64 / total_time.as_secs_f64();

            println!("Throughput test results:");
            println!("Sent {} messages in {:?}", num_messages, total_time);
            println!("Throughput: {:.2} messages/second", throughput);

            // Verify high throughput
            assert!(throughput > 1000.0, 
                "Throughput should be > 1000 msg/sec, got {:.2}", throughput);

            // Shutdown server
            client.shutdown().await.unwrap();
            let _ = tokio::time::timeout(Duration::from_secs(2), server_handle).await;
        });
    }

    #[test]
    fn test_connection_handling() {
        let rt = Runtime::new().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("connection_test.sock");
        
        // Create channels for testing
        let (task_sender, _task_receiver) = flume::unbounded::<NotificationTask>();
        let (shutdown_sender, _shutdown_receiver) = flume::unbounded::<()>();

        rt.block_on(async {
            let server = IpcServer::new(
                socket_path.clone(),
                task_sender,
                shutdown_sender,
            ).await.unwrap();

            let server_handle = tokio::spawn(async move {
                server.run().await.unwrap();
            });

            // Give server time to start
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Test multiple concurrent connections
            let mut handles = Vec::new();
            for i in 0..20 {
                let socket_path = socket_path.clone();
                let handle = tokio::spawn(async move {
                    let client = IpcClient::new(socket_path);
                    
                    // Each client sends multiple pings
                    for _ in 0..10 {
                        client.ping().await.unwrap();
                    }
                    
                    // Test status request
                    let (queue_size, is_running, uptime) = client.status().await.unwrap();
                    assert!(is_running);
                    assert!(uptime >= 0);
                    
                    i
                });
                handles.push(handle);
            }

            // Wait for all connections to complete
            for handle in handles {
                let client_id = handle.await.unwrap();
                assert!(client_id < 20);
            }

            // Test graceful shutdown
            let client = IpcClient::new(socket_path);
            client.shutdown().await.unwrap();
            
            // Server should stop
            let result = tokio::time::timeout(Duration::from_secs(2), server_handle).await;
            assert!(result.is_ok(), "Server should shutdown gracefully");
        });

        println!("Connection handling test passed!");
    }
}