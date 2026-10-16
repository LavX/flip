#![forbid(unsafe_code)]

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind = std::env::var("FLIP_BIND").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port: u16 = std::env::var("FLIP_PORT")
        .unwrap_or_else(|_| "8080".to_owned())
        .parse()?;
    let listener = tokio::net::TcpListener::bind((bind.as_str(), port)).await?;
    axum::serve(listener, flip::api::router()).await?;
    Ok(())
}
