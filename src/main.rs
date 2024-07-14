use futures::FutureExt;
use tokio::{
    pin,
    sync::Notify,
    time::{sleep, Sleep},
};

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

struct Client {
    drop_notifier: Arc<Notify>,
}

impl Client {
    fn poll_broken(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if Box::pin(self.drop_notifier.notified())
            .poll_unpin(cx)
            .is_ready()
        {
            println!("client found session dropped! client is also dropping...");
            return Poll::Ready(());
        }
        Poll::Pending
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        println!("client dropped! notifying session...");
        self.drop_notifier.notify_one(); // we must use notify_one here, because `notify_waiters` has no guarantee that a later call of `notified()` will received this notification.
    }
}

struct Session {
    sleep: Pin<Box<Sleep>>,
    drop_notifier: Arc<Notify>,
}

impl Future for Session {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let sleep = this.sleep.poll_unpin(cx);
        let dropped = this.drop_notifier.notified();
        pin!(dropped);
        let dropped = dropped.poll_unpin(cx);

        if sleep.is_ready() {
            println!("session timeout! notifying client");
            this.drop_notifier.notify_one(); // we must use notify_one here, because `notify_waiters` has no guarantee that a later call of `notified()` will received this notification.
            return Poll::Ready(());
        }

        if dropped.is_ready() {
            println!("session found client dropped! session is also dropping...!");
            return Poll::Ready(());
        }
        Poll::Pending
    }
}

fn serve() -> (Client, Session) {
    let not = Arc::new(Notify::new());
    let client = Client {
        drop_notifier: not.clone(),
    };
    let sess = Session {
        sleep: Box::pin(sleep(Duration::from_secs(5))),
        drop_notifier: not.clone(),
    };
    (client, sess)
}

#[tokio::main]
async fn main() {
    // test client drop
    let (client, session) = serve();
    let join = tokio::spawn(session);
    drop(client);
    let _ = join.await;

    // test session drop
    let (client, session) = serve();
    let join = tokio::spawn(session);

    client.drop_notifier.notified().await;
    let _ = join.await;
}
