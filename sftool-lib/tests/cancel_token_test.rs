use sftool_lib::{CancelToken, Error};

#[test]
fn cancel_token_reports_cancelled_state() {
    let token = CancelToken::new();
    assert!(!token.is_cancelled());
    assert!(token.check_cancelled().is_ok());

    token.cancel();

    assert!(token.is_cancelled());
    assert!(matches!(token.check_cancelled(), Err(Error::Cancelled)));
}
