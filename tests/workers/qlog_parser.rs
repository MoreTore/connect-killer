use connect::app::App;
use loco_rs::prelude::*;
use loco_rs::testing;

use blo::workers::qlog_parser::LogSegmentWorker;
use blo::workers::qlog_parser::LogSegmentWorkerArgs;
use serial_test::serial;


#[tokio::test]
#[serial]
async fn test_run_qlog_parser_worker() {
    let boot = testing::boot_test::<App>().await.unwrap();

    // Execute the worker ensuring that it operates in 'ForegroundBlocking' mode, which prevents the addition of your worker to the background
    assert!(
        LogSegmentWorker::perform_later(&boot.app_context, LogSegmentWorkerArgs {})
            .await
            .is_ok()
    );
    // Include additional assert validations after the execution of the worker
}
