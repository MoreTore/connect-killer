use std::collections::BTreeMap;

use loco_rs::prelude::*;

pub struct Huggingface;
#[async_trait]
impl Task for Huggingface {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "huggingface".to_string(),
            detail: "Task generator".to_string(),
        }
    }
    async fn run(&self, _app_context: &AppContext, _vars: &BTreeMap<String, String>) -> Result<()> {
        println!("Task Huggingface generated");


        Ok(())
    }
}

