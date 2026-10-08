#![forbid(unsafe_code)]

use rivian_host::{AppState, app};
use std::net::{Ipv4Addr, SocketAddrV4};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut port = 0_u16;
    let mut open = true;
    let mut print_launch_url = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => port = args.next().ok_or("--port requires a number")?.parse()?,
            "--no-open" => open = false,
            "--print-launch-url" => print_launch_url = true,
            "--version" => {
                println!(
                    "My Rivian Data {} (offline prototype)",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "My Rivian Data - offline prototype\nYour vehicles. Your data.\n\nUsage: my-rivian-data [--port NUMBER] [--no-open] [--print-launch-url]\n\nDefault: random IPv4 loopback port; opens your browser.\n--print-launch-url explicitly prints a sensitive, single-use, five-minute link.\nThis build does not accept Rivian credentials or contact Rivian."
                );
                return Ok(());
            }
            _ => return Err("Unknown argument; use --help".into()),
        }
    }
    let listener =
        tokio::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).await?;
    let (state, bootstrap) = AppState::new(listener.local_addr()?.port())?;
    let launch_url = format!("{}/#bootstrap={bootstrap}", state.origin());
    println!("My Rivian Data: offline demo at {}", state.origin());
    if print_launch_url {
        println!("Launch URL (keep private): {launch_url}");
    }
    if open && webbrowser::open(&launch_url).is_err() {
        eprintln!(
            "Could not open a browser. Restart with --no-open --print-launch-url for an explicit one-use link."
        );
    }
    axum::serve(listener, app(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
