#[macro_export]
#[cfg(__hyper_impl_trait_available)]
macro_rules! impl_trait {
    (ty: $($t:tt)+) => {
        impl $($t)+
    };
    (e: $e:expr) => {
        $e
    }
}

#[macro_export]
#[cfg(not(__hyper_impl_trait_available))]
macro_rules! impl_trait {
    (ty: $($t:tt)+) => {
        Box<$($t)+>
    };
    (e: $e:expr) => {
        Box::new($e)
    }
}
