use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match lumenize::cli::run().await {
        Ok(status) => ExitCode::from(u8::try_from(status).unwrap_or(1)),
        Err(error) => {
            eprintln!("lumenize: {error}");
            ExitCode::FAILURE
        }
    }
}
