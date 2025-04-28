#[macro_export]
macro_rules! handle_result {
    ($sender:expr, $res:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => {
                let res = $crate::status::handle_error(&$sender, e.into());
                match res {
                    $crate::handle_error::ErrorBranch::Break => break,
                    $crate::handle_error::ErrorBranch::Continue => continue,
                }
            }
        }
    };
}

pub enum ErrorBranch {
    Break,
    Continue,
}
