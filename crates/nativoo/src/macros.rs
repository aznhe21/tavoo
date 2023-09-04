/// 値がエラーの際にラベル付きブロックを抜ける。
macro_rules! tri {
    ($label:lifetime, $v:expr) => {
        match $v {
            Ok(val) => val,
            Err(err) => break $label Err(err.into()),
        }
    };
}
