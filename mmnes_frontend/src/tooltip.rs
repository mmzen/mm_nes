

pub trait ToolTip {
    fn tooltip(str: &str) -> Option<&'static str>;
}