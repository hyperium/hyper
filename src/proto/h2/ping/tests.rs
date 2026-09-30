use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::rt::Timer;

#[derive(Debug)]
struct TestTimer;

struct TestSleep(Pin<Box<tokio::time::Sleep>>);

impl Future for TestSleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<()> {
        self.0.as_mut().poll(cx)
    }
}

impl Sleep for TestSleep {}

impl Timer for TestTimer {
    fn now(&self) -> Instant {
        tokio::time::Instant::now().into_std()
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>> {
        Box::pin(TestSleep(Box::pin(tokio::time::sleep(duration))))
    }

    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>> {
        Box::pin(TestSleep(Box::pin(tokio::time::sleep_until(
            deadline.into(),
        ))))
    }
}

#[derive(Debug, Default)]
struct Observer(AtomicUsize);

impl KeepAliveObserver for Observer {
    fn on_reuse_timeout(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn waiting() -> (KeepAlive, Arc<Observer>) {
    let timer = Time::Timer(Arc::new(TestTimer));
    let observer = Arc::new(Observer::default());
    let ka = KeepAlive {
        interval: Duration::from_secs(10),
        timeout: Duration::from_secs(60),
        reuse_timeout: Some(Duration::from_secs(5)),
        reuse_sleep: Some(timer.sleep(Duration::from_secs(5))),
        observer: Some(observer.clone()),
        while_idle: true,
        state: KeepAliveState::PingSent,
        sleep: timer.sleep(Duration::from_secs(60)),
        timer,
    };
    (ka, observer)
}

#[tokio::test(start_paused = true)]
async fn reuse_timer_wakes_without_io_and_does_not_replace_hard_timer() {
    let (mut ka, observer) = waiting();
    let start = tokio::time::Instant::now();
    let notification = std::future::poll_fn(|cx| {
        assert!(ka.maybe_timeout(cx).is_ok());
        match ka.poll_reuse_timeout(cx) {
            Some(observer) => Poll::Ready(observer),
            None => Poll::Pending,
        }
    })
    .await;
    assert_eq!(start.elapsed(), Duration::from_secs(5));
    notification.on_reuse_timeout();
    assert_eq!(observer.0.load(Ordering::SeqCst), 1);

    // Continue polling both paths; only the original hard deadline can complete.
    std::future::poll_fn(|cx| {
        assert!(ka.poll_reuse_timeout(cx).is_none());
        if ka.maybe_timeout(cx).is_err() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    assert_eq!(start.elapsed(), Duration::from_secs(60));
    assert_eq!(observer.0.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn only_keepalive_waiting_phase_can_report_reuse_timeout() {
    let (mut ka, observer) = waiting();
    ka.state = KeepAliveState::Scheduled(ka.timer.now());
    tokio::time::advance(Duration::from_secs(5)).await;
    std::future::poll_fn(|cx| {
        assert!(ka.poll_reuse_timeout(cx).is_none());
        assert!(ka.maybe_timeout(cx).is_ok());
        Poll::Ready(())
    })
    .await;
    assert_eq!(observer.0.load(Ordering::SeqCst), 0);
}
