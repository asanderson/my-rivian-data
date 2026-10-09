#![forbid(unsafe_code)]

use rivian_host::{AppState, app};
use std::net::{Ipv4Addr, SocketAddrV4};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut demo = false;
    let mut port = 0_u16;
    let mut open = true;
    let mut print_launch_url = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => demo = true,
            "--port" => port = args.next().ok_or("--port requires a number")?.parse()?,
            "--no-open" => open = false,
            "--print-launch-url" => print_launch_url = true,
            "--version" => {
                println!("My Rivian Data {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "My Rivian Data\nYour vehicles. Your data.\n\nUsage: my-rivian-data [--demo] [--port NUMBER] [--no-open] [--print-launch-url]\n\nDefault: random IPv4 loopback port; opens your browser.\n--print-launch-url explicitly prints a sensitive, single-use, five-minute link.\nDefault mode connects to Rivian after you sign in. Use --demo for offline sample data."
                );
                return Ok(());
            }
            _ => return Err("Unknown argument; use --help".into()),
        }
    }
    let listener =
        tokio::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).await?;
    let (state, bootstrap) = if demo {
        AppState::new_demo(listener.local_addr()?.port())?
    } else {
        AppState::new(listener.local_addr()?.port())?
    };
    let launch_url = format!("{}/#bootstrap={bootstrap}", state.origin());
    println!(
        "My Rivian Data: {} at {}",
        if demo {
            "sample mode"
        } else {
            "Rivian account access"
        },
        state.origin()
    );
    let expiry_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            expiry_state.expire_sessions();
        }
    });
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
