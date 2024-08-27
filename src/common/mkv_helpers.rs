use::std::env;

pub fn list_keys_starting_with(str: &str) -> String {
  let mkv_endpoint = env::var("MKV_ENDPOINT").expect("MKV_ENDPOINT env variable not set");
  format!("{}/{}?list", mkv_endpoint, str)
}

pub fn get_mkv_file_url(file: &str) -> String {
  let mkv_endpoint = env::var("MKV_ENDPOINT").expect("MKV_ENDPOINT env variable not set");
  format!("{}/{}", mkv_endpoint, file)
}