use reqwest;

pub fn post(url: &str, body: &str) -> Result<(String, u16), String> {
    let client = reqwest::Client::new();
    match client
        .post(url)
        .body(String::from(body))
        .header("Content-type", "application/json")
        .send()
    {
        Ok(mut o) => {
            let msg = format!("{}", o.text().unwrap());
            let status = o.status().as_u16();
            Ok((msg, status))
        }
        Err(e) => Err(format!("{}", e)),
    }
}
