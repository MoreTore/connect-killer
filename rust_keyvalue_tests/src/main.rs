use reqwest::{Client, StatusCode};
use hex::encode;
use rand::{RngCore, rngs::OsRng};
use tokio;

#[tokio::main]
async fn main() {
    let client = Client::new();
    let key = get_fresh_key().await;
    
    // Test PUT, GET, DELETE sequence
    test_put_get_delete(&client, &key).await;
}

async fn get_fresh_key() -> String {
    let mut bytes = [0u8; 10];
    OsRng.fill_bytes(&mut bytes);
    format!("http://localhost:3000/swag-{}", encode(bytes))
}

async fn test_put_get_delete(client: &Client, key: &str) {
    // PUT request
    let res = client.put(key)
                    .body("onyou")
                    .send()
                    .await
                    .expect("Failed to execute request.");
    assert_eq!(res.status(), StatusCode::CREATED);

    // GET request
    let res = client.get(key)
                    .send()
                    .await
                    .expect("Failed to execute request.");
    assert_eq!(res.status(), StatusCode::OK);
    let text = res.text().await.expect("Failed to read response text.");
    assert_eq!(text, "onyou");

    // DELETE request
    let res = client.delete(key)
                    .send()
                    .await
                    .expect("Failed to execute request.");
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}



