#[macro_export]
macro_rules! cancel {
    ($request:ident, $hash: expr, $net_id:expr, $err:expr) => {
        if let Err(e) = $request.cancel() {
            error!(
                "Request cancellation failed: {:?}, {:?}, {:?}",
                $err, e, $request
            );
            $crate::RequestStatuses::<T>::insert($net_id, $hash, RequestStatus::Broken($err, e));
            Self::deposit_event(Event::CancellationFailed($hash));
            // Such errors should not occur in general, but we check it in tests, anyway.
            #[cfg(not(test))]
            debug_assert!(false, "unexpected cancellation error {:?}", e);
        } else {
            $crate::RequestStatuses::<T>::insert($net_id, $hash, RequestStatus::Failed($err));
        }
    };
}
