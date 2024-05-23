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