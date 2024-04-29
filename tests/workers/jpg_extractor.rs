use connect::app::App;
use loco_rs::prelude::*;
use loco_rs::testing;

use blo::workers::jpg_extractor::JpgExtractorWorker;
use blo::workers::jpg_extractor::JpgExtractorWorkerArgs;
use serial_test::serial;


#[tokio::test]
#[serial]
async fn test_run_jpg_extractor_worker() {
    let boot = testing::boot_test::<App>().await.unwrap();

    // Execute the worker ensuring that it operates in 'ForegroundBlocking' mode, which prevents the addition of your worker to the background
    assert!(
        JpgExtractorWorker::perform_later(&boot.app_context, JpgExtractorWorkerArgs {})
            .await
            .is_ok()
    );
    // Include additional assert validations after the execution of the worker
}
