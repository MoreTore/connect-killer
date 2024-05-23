pub const MKV_ENDPOINT: &str = "http://localhost:3000";

pub fn list_keys_starting_with(str: &str) -> String {
  format!("{}/{}?list", MKV_ENDPOINT, str)
}

pub fn get_mkv_file_url(file: &String) -> String {
    format!("{}/{}", MKV_ENDPOINT, file)
}