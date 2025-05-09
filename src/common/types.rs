#[derive(Debug)]
enum FileType {
    Rlog,
    Qlog,
    Qcamera,
    Fcamera,
    Dcamera,
    Ecamera,
    Invalid,
}

impl FileType {
    fn from_str(file: &str) -> FileType {
        match file {
            "rlog.bz2" => FileType::Rlog,
            "qlog.bz2" => FileType::Qlog,
            "qcamera.ts" => FileType::Qcamera,
            "fcamera.hevc" => FileType::Fcamera,
            "dcamera.hevc" => FileType::Dcamera,
            "ecamera.hevc" => FileType::Ecamera,
            _ => FileType::Invalid,
        }
    }
}

// Function to get the event type name as a string based on the WhichReader
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cereal/generated_event_type_names.rs"));

pub fn get_event_name(event_type: &LogEvent::WhichReader) -> String {
    generated_event_type_name(event_type)
}