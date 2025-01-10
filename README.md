# Rex Axum Scheduler

A asynchronous task scheduler for Rust, built on top of tokio runtime. It provides flexible cron expression support for scheduling tasks with atomic operation guarantees.

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
