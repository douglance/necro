#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    necro::build_cli().serve().await
}
