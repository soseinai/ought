use serde_json::Value;

/// Handler for MCP tool invocations.
///
/// Each tool maps to an `ought` CLI command and returns structured JSON.
pub struct ToolHandler {
    // will hold references to config, specs, etc.
}

impl ToolHandler {
    pub fn ought_run(&self, _args: Value) -> anyhow::Result<Value> {
        todo!()
    }

    pub fn ought_generate(&self, _args: Value) -> anyhow::Result<Value> {
        todo!()
    }

    pub fn ought_check(&self, _args: Value) -> anyhow::Result<Value> {
        todo!()
    }

    pub fn ought_inspect(&self, _args: Value) -> anyhow::Result<Value> {
        todo!()
    }

    pub fn ought_status(&self, _args: Value) -> anyhow::Result<Value> {
        todo!()
    }

    pub fn ought_survey(&self, _args: Value) -> anyhow::Result<Value> {
        todo!()
    }

    pub fn ought_audit(&self, _args: Value) -> anyhow::Result<Value> {
        todo!()
    }

    pub fn ought_blame(&self, _args: Value) -> anyhow::Result<Value> {
        todo!()
    }

    pub fn ought_bisect(&self, _args: Value) -> anyhow::Result<Value> {
        todo!()
    }
}
