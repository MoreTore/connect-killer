use serde::{Deserialize, Serialize};
use loco_rs::prelude::*;
use capnp::message::ReaderOptions;
use std::fs::File;
use std::io::{Read, Write, BufReader, BufWriter, Cursor};
use std::path::Path;
use anyhow::{Result, Context};
use bzip2::read::BzDecoder;
use crate::cereal::log_capnp;


pub struct QlogParserWorker {
    pub ctx: AppContext,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct QlogParserWorkerArgs {
    pub path: String,
    pub output: String,
}

impl worker::AppWorker<QlogParserWorkerArgs> for QlogParserWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }
}

#[async_trait]
impl worker::Worker<QlogParserWorkerArgs> for QlogParserWorker {
    async fn perform(&self, args: QlogParserWorkerArgs) -> worker::Result<()> {
        println!("=================QlogParser=======================");
        let path = Path::new(&args.path);

        // Here, properly handle the Result returned by File::open using `?`
        let file = File::open(path)
            .with_context(|| format!("Failed to open file: {:?}", path)).unwrap();

        let mut data = Vec::new();
        if path.extension() == Some(std::ffi::OsStr::new("bz2")) {
            // `file` is directly used after confirming it is Ok above
            let mut bz2_decoder = BzDecoder::new(file);
            bz2_decoder.read_to_end(&mut data).unwrap();
        } else {
            let mut buf_reader = BufReader::new(file);
            buf_reader.read_to_end(&mut data).unwrap();
        }

        println!("Unlogging!");
        let mut cursor = Cursor::new(&data);
        // Open output file for writing
        
        let out_file = File::create(format!("{}", &args.output)).unwrap();
        let mut writer = BufWriter::new(out_file);
        while let Ok(message_reader) = capnp::serialize::read_message(&mut cursor, ReaderOptions::default()) {
            let event = message_reader.get_root::<log_capnp::event::Reader>().unwrap();
            writeln!(writer, "{:#?}", event).unwrap();
        }
        println!("All done unlogging: {}", args.output);
        Ok(())
    }
}