use rex_axum_sdk::scheduler::Scheduler;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep; // 請替換成你的 crate 名稱

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 建立一個計數器來追蹤任務執行次數
    let counter = Arc::new(AtomicUsize::new(0));
    let mut scheduler = Scheduler::new().await?;

    println!("建立排程任務...");

    // 每分鐘執行一次的任務
    let counter_clone = counter.clone();
    scheduler
        .add_task("* * * * * *", move || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst) + 1;
                println!("每秒任務執行第 {} 次", count);
            }
        })
        .await?;

    // 每 5 秒執行一次的任務
    let counter_clone = counter.clone();
    scheduler
        .add_task("*/5 * * * * *", move || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.load(Ordering::SeqCst);
                println!("5秒報告：目前總執行次數 = {}", count);
            }
        })
        .await?;

    println!("啟動排程器...");
    scheduler.start().await?;

    // 讓程式執行 30 秒
    println!("排程器將執行 30 秒...");
    sleep(Duration::from_secs(30)).await;

    // 停止排程器
    println!("停止排程器...");
    scheduler.stop().await?;

    let final_count = counter.load(Ordering::SeqCst);
    println!("排程器已停止。總執行次數: {}", final_count);

    Ok(())
}
