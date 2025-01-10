# Rex Axum Scheduler
A comprehensive Rust SDK providing essential tools for web service development, featuring Firebase integration, task scheduling, SQLx utilities, and more.
Features

Firebase Authentication: JWT validation and claims management
FCM Messaging: Firebase Cloud Messaging integration
Task Scheduler: Cron-based task scheduling with async support
SQLx Extensions: Enhanced PostgreSQL query builder and execution utilities

## Quick Start

```rust
use rex_axum_sdk::scheduler::Scheduler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new().await?;
    scheduler
        .add_task("* * * * * *", || async {
            println!("Task executed");
        })
        .await?;
    scheduler.start().await?;
    Ok(())
}
```
