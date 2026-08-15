// http_post.vbr — POST a JSON body with request headers.
// 
// This is the shape of an LLM API call: a JSON body, a Content-Type, and an
// `Authorization: Bearer` token. Headers are a HashMap<String, String> (VB's
// Scripting.Dictionary); pass an empty one for no custom headers.

use std::collections::HashMap;

fn vbr_main() -> Result<(), String> {
    let key: String = "sk-demo-key".to_string();
    let body: String = "{\"model\": \"demo\", \"prompt\": \"hello\"}".to_string();
    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert("Authorization".to_string(), format!("Bearer {}", key));
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    let mut reply: String;
    reply = match Http::post("https://api.example.com/v1/complete", &body, headers) {
        Ok(__vbr_ok) => __vbr_ok,
        Err(message) => {
            println!("request failed: {}", message);
            return Ok(());
        }
    };
    println!("got {} bytes", reply.len());
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
