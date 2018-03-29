use futures::executor::Executor;

pub(crate) trait CloneBoxedExecutor: Executor + Send + Sync {
    fn clone_boxed(&self) -> Box<CloneBoxedExecutor + Send + Sync>;
}

impl<E: Executor + Clone + Send + Sync + 'static> CloneBoxedExecutor for E {
    fn clone_boxed(&self) -> Box<CloneBoxedExecutor + Send + Sync> {
        Box::new(self.clone())
    }
}

impl Clone for Box<CloneBoxedExecutor> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}
