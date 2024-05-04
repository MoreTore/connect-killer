use connect::app::App;
use loco_rs::prelude::*;
use loco_rs::testing;

use blo::workers::bootlog_parser::BootlogParserWorker;
use blo::workers::bootlog_parser::BootlogParserWorkerArgs;
use serial_test::serial;


#[tokio::test]
#[serial]
async fn test_run_bootlog_parser_worker() {
    let boot = testing::boot_test::<App>().await.unwrap();

    // Execute the worker ensuring that it operates in 'ForegroundBlocking' mode, which prevents the addition of your worker to the background
    assert!(
        BootlogParserWorker::perform_later(&boot.app_context, BootlogParserWorkerArgs {})
            .await
            .is_ok()
    );
    // Include additional assert validations after the execution of the worker
}
