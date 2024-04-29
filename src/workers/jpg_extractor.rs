use serde::{Deserialize, Serialize};
use loco_rs::prelude::*;

pub struct JpgExtractorWorker {
    pub ctx: AppContext,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct JpgExtractorWorkerArgs {
}

impl worker::AppWorker<JpgExtractorWorkerArgs> for JpgExtractorWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }
}

#[async_trait]
impl worker::Worker<JpgExtractorWorkerArgs> for JpgExtractorWorker {
    async fn perform(&self, _args: JpgExtractorWorkerArgs) -> worker::Result<()> {
        println!("=================JpgExtractor=======================");
        // TODO: Some actual work goes here...
        Ok(())
    }
}
