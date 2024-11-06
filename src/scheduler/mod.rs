use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio_cron_scheduler::{Job, JobScheduler};
pub type JobFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type JobCallback = Box<dyn Fn() -> JobFuture + Send + Sync>;

pub struct CronJob {
    pub cron_expr: String,
    pub callback: JobCallback,
}

impl CronJob {
    pub fn new<F>(cron_expr: impl Into<String>, callback: F) -> Self
    where
        F: Fn() -> JobFuture + Send + Sync + 'static,
    {
        Self {
            cron_expr: cron_expr.into(),
            callback: Box::new(callback),
        }
    }
}

pub struct Scheduler {
    scheduler: JobScheduler,
    is_running: Arc<AtomicBool>, // 新增狀態控制
}

impl Scheduler {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self {
            scheduler,
            is_running: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_running.store(true, Ordering::SeqCst);
        self.scheduler.start().await?;
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_running.store(false, Ordering::SeqCst);
        // 等待一小段時間確保所有任務都看到停止信號
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.scheduler.shutdown().await?;
        Ok(())
    }

    pub async fn add_task<F, Fut>(
        &self,
        cron_expr: &str,
        task: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn() -> Fut + Send + Sync + 'static + Clone,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let is_running = self.is_running.clone();

        let job = Job::new_async(cron_expr, move |_, _| {
            let is_running = is_running.clone();
            let task = task.clone(); // 如果 F 不能 clone，需要用 Arc 包裝
            Box::pin(async move {
                if is_running.load(Ordering::SeqCst) {
                    task().await;
                }
            })
        })?;

        self.scheduler.add(job).await?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::sleep;

    // 測試建立排程器
    #[tokio::test]
    async fn test_scheduler_creation() {
        let scheduler = Scheduler::new().await;
        assert!(scheduler.is_ok(), "應該能夠成功建立排程器");
    }

    // 測試基本的任務執行
    #[tokio::test]
    async fn test_basic_task_execution() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let mut scheduler = Scheduler::new().await.unwrap();

        scheduler
            .add_task("* * * * * *", move || {
                let counter = counter_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await
            .unwrap();

        scheduler.start().await.unwrap();
        sleep(Duration::from_secs(2)).await;
        scheduler.stop().await.unwrap();

        let final_count = counter.load(Ordering::SeqCst);
        println!("最終執行次數: {}", final_count);
        assert!(final_count > 0, "任務應該至少執行一次");
    }

    // 測試多個任務的並行執行
    #[tokio::test]
    async fn test_concurrent_tasks() {
        let counter1 = Arc::new(AtomicUsize::new(0));
        let counter2 = Arc::new(AtomicUsize::new(0));
        let mut scheduler = Scheduler::new().await.unwrap();

        // 第一個任務每秒執行
        let counter1_clone = counter1.clone();
        scheduler
            .add_task("* * * * * *", move || {
                let counter = counter1_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await
            .unwrap();

        // 第二個任務每 2 秒執行
        let counter2_clone = counter2.clone();
        scheduler
            .add_task("*/2 * * * * *", move || {
                let counter = counter2_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await
            .unwrap();

        scheduler.start().await.unwrap();
        sleep(Duration::from_secs(3)).await;
        scheduler.stop().await.unwrap();

        let count1 = counter1.load(Ordering::SeqCst);
        let count2 = counter2.load(Ordering::SeqCst);

        assert!(count1 > count2, "第一個任務應該執行更多次");
        println!("任務1執行次數: {}, 任務2執行次數: {}", count1, count2);
    }

    // 測試排程器的停止功能
    #[tokio::test]
    async fn test_scheduler_stop() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let mut scheduler = Scheduler::new().await.unwrap();

        // 新增任務
        scheduler
            .add_task("* * * * * *", move || {
                let counter = counter_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    println!("當前計數: {}", counter.load(Ordering::SeqCst));
                }
            })
            .await
            .unwrap();

        // 啟動排程器
        scheduler.start().await.unwrap();

        // 等待確保至少執行一次
        println!("等待任務執行...");
        sleep(Duration::from_secs(2)).await;

        let count_before_stop = counter.load(Ordering::SeqCst);
        println!("停止前計數: {}", count_before_stop);

        // 停止排程器
        scheduler.stop().await.unwrap();

        // 再等待一段時間
        println!("等待確認停止...");
        sleep(Duration::from_secs(2)).await;

        let count_after_stop = counter.load(Ordering::SeqCst);
        println!("停止後計數: {}", count_after_stop);

        assert_eq!(
            count_before_stop, count_after_stop,
            "停止後不應該繼續執行任務"
        );
    }
    // 測試無效的 cron 表達式
    #[tokio::test]
    async fn test_invalid_cron_expression() {
        let scheduler = Scheduler::new().await.unwrap();

        let result = scheduler.add_task("invalid", || async {}).await;

        assert!(result.is_err(), "無效的 cron 表達式應該返回錯誤");
        if let Err(e) = result {
            println!("預期的錯誤: {}", e);
        }
    }

    // 測試重複啟動
    #[tokio::test]
    async fn test_double_start() {
        let mut scheduler = Scheduler::new().await.unwrap();

        assert!(scheduler.start().await.is_ok(), "第一次啟動應該成功");
        // 第二次啟動應該要有適當的錯誤處理
        match scheduler.start().await {
            Ok(_) => println!("注意：排程器允許重複啟動"),
            Err(e) => println!("預期的重複啟動錯誤: {}", e),
        }

        scheduler.stop().await.unwrap();
    }

    // 測試任務執行時的錯誤處理
    #[tokio::test]
    async fn test_task_error_handling() {
        let mut scheduler = Scheduler::new().await.unwrap();

        // 新增一個會拋出錯誤的任務
        scheduler
            .add_task("* * * * * *", || async {
                panic!("測試任務錯誤");
            })
            .await
            .unwrap();

        scheduler.start().await.unwrap();
        sleep(Duration::from_secs(2)).await;

        // 確保排程器依然在運行
        let result = scheduler.stop().await;
        assert!(result.is_ok(), "即使任務出錯，排程器也應該能正常停止");
    }
}
