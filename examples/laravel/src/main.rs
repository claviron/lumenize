use lumenize::{Options, Runner};

#[tokio::main]
async fn main() -> lumenize::Result<()> {
    Runner::new("laravel-example")
        .with_options(Options::from_args()?)
        .run()
        .await
}
