/// Options controlling report output.
#[derive(Debug, Clone, Default)]
pub struct ReportOptions {
    pub quiet: bool,
    pub color: ColorChoice,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}
