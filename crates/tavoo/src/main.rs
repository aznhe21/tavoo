mod app;
mod message;
mod scheme;

fn main() -> anyhow::Result<()> {
    app::run()
}
